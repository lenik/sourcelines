use std::fs::{self, File};
use std::io::{self, BufRead, Read};
use std::path::Path;

use clap::{ArgGroup, Parser};
use globset::{Glob, GlobSet, GlobSetBuilder};
use sourcelines::{CommentSyntax, detect_comment_syntax, detect_language};

#[derive(Default, Debug, Clone)]
struct Stats {
    actual_loc: usize,
    raw_loc: usize,
    words: usize,
    chars: usize,
    bytes: usize,
}

#[derive(Parser, Debug)]
#[command(
    name = "sourcelines",
    version,
    about = "Count source code statistics: actual lines of code, raw lines, words, chars, bytes.",
    long_about = None,
    after_help = "For more details, see the man page or sourcelines.1.md."
)]
#[command(group(ArgGroup::new("columns").multiple(true)))]
struct Cli {
    /// Recursively process directories
    #[arg(short = 'r', long = "recursive")]
    recursive: bool,

    /// Output summary line at the end
    #[arg(short = 's', long = "sum")]
    sum: bool,

    /// Verbose output: with -s, print all file stats; for directories, print per-language summary
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Output with ANSI coloring
    #[arg(short = 'C', long = "color")]
    color: bool,

    /// Exclude files/directories matching these wildcard patterns (can be used multiple times)
    #[arg(long = "exclude", value_name = "WILDCARD", num_args = 0.., default_value = "")]
    exclude: Vec<String>,

    /// Include files/directories matching these wildcard patterns (can be used multiple times)
    #[arg(long = "include", value_name = "WILDCARD", num_args = 0.., default_value = "")]
    include: Vec<String>,

    /// Show actual klocs (actual lines/1000)
    #[arg(short = 'k', long = "actual-klocs", group = "columns")]
    actual_klocs: bool,
    /// Show actual loc
    #[arg(short = 'l', long = "actual-loc", group = "columns")]
    actual_loc: bool,
    /// Show raw klocs (raw lines/1000)
    #[arg(short = 'K', long = "raw-klocs", group = "columns")]
    raw_klocs: bool,
    /// Show raw loc
    #[arg(short = 'R', long = "raw-locs", group = "columns")]
    raw_loc: bool,
    /// Follow symlinks when recursively processing directories
    #[arg(short = 'L', long = "follow-symlinks")]
    follow_symlinks: bool,
    /// Show word count
    #[arg(short = 'w', long = "words", group = "columns")]
    words: bool,
    /// Show char count
    #[arg(short = 'c', long = "chars", group = "columns")]
    chars: bool,
    /// Show byte count
    #[arg(short = 'b', long = "bytes", group = "columns")]
    bytes: bool,

    /// Files or directories to process
    #[arg(required = false)]
    files: Vec<String>,
}

fn main() {
    let mut cli = Cli::parse();
    // If no files provided, default to -rv .
    if cli.files.is_empty() {
        cli.files = vec![".".to_string()];
        cli.recursive = true;
        cli.verbose = true;
    }
    let show_actual_klocs = cli.actual_klocs;
    let show_actual_loc = cli.actual_loc;
    let show_raw_klocs = cli.raw_klocs;
    let show_raw_loc = cli.raw_loc;
    let mut show_words = cli.words;
    let mut show_chars = cli.chars;
    let mut show_bytes = cli.bytes;
    let recursive = cli.recursive;
    let show_sum = cli.sum;
    let verbose = cli.verbose;
    let color = cli.color;
    let follow_symlinks = cli.follow_symlinks;
    let files = &cli.files;

    // Default exclude patterns
    let default_excludes = vec![
        "*~",
        "~*",
        "*$",
        "$*",
        ".git",
        ".svn",
        "*.bak",
        "*.lock",
        "*.log",
        "*.tmp",
        "_build",
        "build",
        "builddir",
        "node_modules",
        "target",
    ];
    // Build exclude set
    let mut exclude_patterns = default_excludes
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    exclude_patterns.extend(cli.exclude.iter().cloned());
    // Remove from exclude if present in include
    let include_patterns = cli.include.clone();
    for inc in &include_patterns {
        exclude_patterns.retain(|e| e != inc);
    }
    let exclude_set = build_globset(&exclude_patterns);
    let include_set = if !include_patterns.is_empty() {
        Some(build_globset(&include_patterns))
    } else {
        None
    };

    // By default, show loc, raw loc, words, chars, bytes (not klocs)
    let show_actual_klocs = show_actual_klocs;
    let mut show_actual_loc = show_actual_loc;
    let show_raw_klocs = show_raw_klocs;
    let mut show_raw_loc = show_raw_loc;
    let show_default = !(show_actual_klocs
        || show_actual_loc
        || show_raw_klocs
        || show_raw_loc
        || show_words
        || show_chars
        || show_bytes);

    if show_default {
        show_actual_loc = true;
        show_raw_loc = true;
        show_words = true;
        show_chars = true;
        show_bytes = true;
    } else {
        if show_actual_klocs && show_actual_loc {
            show_actual_loc = false;
        }
        if show_raw_klocs && show_raw_loc {
            show_raw_loc = false;
        }
    }

    let mut sum = Stats::default();
    let mut per_lang_sum: std::collections::HashMap<String, Stats> =
        std::collections::HashMap::new();
    let mut file_stats: Vec<(Stats, String, String, bool)> = Vec::new(); // (stats, lang, arg, is_dir)
    for arg in files {
        let path = Path::new(arg);
        if path.is_dir() {
            let (dir_stats, lang_map) =
                process_dir_lang_filtered(path, recursive, follow_symlinks, &exclude_set, include_set.as_ref());
            sum = add_stats(sum, dir_stats.clone());
            // Save per-language sums for verbose mode
            for (lang, stats) in lang_map.iter() {
                let entry = per_lang_sum.entry(lang.clone()).or_default();
                *entry = add_stats(entry.clone(), stats.clone());
            }
            file_stats.push((dir_stats, "*".to_string(), arg.clone(), true));
        } else {
            let stats = process_file(path);
            sum = add_stats(sum, stats.clone());
            let lang = detect_language(path);
            file_stats.push((stats, lang, arg.clone(), false));
        }
    }

    if verbose || !show_sum {
        // Print all file stats
        for (stats, lang, arg, is_dir) in &file_stats {
            print_stats(
                stats,
                lang,
                Some(arg.as_str()),
                show_actual_klocs,
                show_actual_loc,
                show_raw_klocs,
                show_raw_loc,
                show_words,
                show_chars,
                show_bytes,
                false,
                color,
            );
            if *is_dir && verbose {
                // For directories, print per-language sum
                let path = Path::new(arg);
                let (_, lang_map) =
                    process_dir_lang_filtered(path, recursive, follow_symlinks, &exclude_set, include_set.as_ref());

                // Sort grouped (per-language) results by the first visible column in descending order
                let first_col_value = |s: &Stats| -> usize {
                    if show_actual_klocs {
                        s.actual_loc
                    } else if show_actual_loc {
                        s.actual_loc
                    } else if show_raw_klocs {
                        s.raw_loc
                    } else if show_raw_loc {
                        s.raw_loc
                    } else if show_words {
                        s.words
                    } else if show_chars {
                        s.chars
                    } else {
                        s.bytes
                    }
                };

                let mut items: Vec<(&String, &Stats)> = lang_map.iter().collect();
                // Filter out languages with zero counts
                items.retain(|(_, stats)| {
                    stats.actual_loc > 0
                        || stats.raw_loc > 0
                        || stats.words > 0
                        || stats.chars > 0
                        || stats.bytes > 0
                });
                items.sort_by(|(la, sa), (lb, sb)| {
                    let ka = first_col_value(sa);
                    let kb = first_col_value(sb);
                    kb.cmp(&ka).then_with(|| la.cmp(lb))
                });

                for (lang, stats) in items.into_iter() {
                    print_stats(
                        stats,
                        lang,
                        None,
                        show_actual_klocs,
                        show_actual_loc,
                        show_raw_klocs,
                        show_raw_loc,
                        show_words,
                        show_chars,
                        show_bytes,
                        false,
                        color,
                    );
                }
            }
        }
    }

    // Print output according to -s and -v
    if show_sum {
        // Always print global sum at end
        print_stats(
            &sum,
            "*",
            Some("(sum)"),
            show_actual_klocs || (show_default && !show_actual_loc),
            show_actual_loc || (show_default && !show_actual_klocs),
            show_raw_klocs || (show_default && !show_raw_loc),
            show_raw_loc || (show_default && !show_raw_klocs),
            show_words || show_default,
            show_chars || show_default,
            show_bytes || show_default,
            true,
            color,
        );
    }

    // Like process_dir, but returns (total_stats, per_language_map), with filtering
    fn process_dir_lang_filtered(
        path: &Path,
        recursive: bool,
        follow_symlinks: bool,
        exclude_set: &GlobSet,
        include_set: Option<&GlobSet>,
    ) -> (Stats, std::collections::HashMap<String, Stats>) {
        let mut total = Stats::default();
        let mut lang_map: std::collections::HashMap<String, Stats> =
            std::collections::HashMap::new();
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return (total, lang_map),
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let is_excluded =
                exclude_set.is_match(fname) && include_set.map_or(true, |inc| !inc.is_match(fname));
            if is_excluded {
                continue;
            }
            // Check if it's a symlink
            let is_symlink = fs::symlink_metadata(&p)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            
            // Skip symlinks if follow_symlinks is false
            if is_symlink && !follow_symlinks {
                continue;
            }
            
            if recursive && p.is_dir() {
                let (dir_stats, dir_lang_map) =
                    process_dir_lang_filtered(&p, true, follow_symlinks, exclude_set, include_set);
                total = add_stats(total, dir_stats.clone());
                for (lang, stats) in dir_lang_map {
                    let entry = lang_map.entry(lang).or_default();
                    *entry = add_stats(entry.clone(), stats);
                }
            } else if p.is_file() {
                let stats = process_file(&p);
                let lang = detect_language(&p);
                let entry = lang_map.entry(lang).or_default();
                *entry = add_stats(entry.clone(), stats.clone());
                total = add_stats(total, stats);
            }
        }
        (total, lang_map)
    }

    fn build_globset(patterns: &[String]) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        for pat in patterns {
            // Accept both literal and glob patterns
            let g = Glob::new(pat).unwrap_or_else(|_| Glob::new(&glob_escape(pat)).unwrap());
            builder.add(g);
        }
        builder.build().unwrap()
    }

    fn glob_escape(s: &str) -> String {
        // Escape all special glob characters
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '*' | '?' | '[' | ']' | '{' | '}' | '!' | '(' | ')' | '|' | '^' | '$' | '+'
                | '.' | '#' => {
                    out.push('[');
                    out.push(c);
                    out.push(']');
                }
                _ => out.push(c),
            }
        }
        out
    }
}

fn print_stats(
    stats: &Stats,
    lang: &str,
    filename: Option<&str>,
    show_actual_klocs: bool,
    show_actual_loc: bool,
    show_raw_klocs: bool,
    show_raw_loc: bool,
    show_words: bool,
    show_chars: bool,
    show_bytes: bool,
    is_sum: bool,
    color: bool,
) {
    let mut out = String::new();
    let fname = filename.unwrap_or("");

    let cyan = "\x1b[36m";
    let green = "\x1b[32m";
    let yellow = "\x1b[33m";
    let magenta = "\x1b[35m";
    let blue = "\x1b[34m";
    // let lightgray = "\x1b[35m";
    let lightgray = "\x1b[2:38m";
    let reset = "\x1b[0m";

    if color && filename.is_some() {
        if show_actual_klocs {
            out += &format!("{}{:>8.3}{} ", cyan, stats.actual_loc as f64 / 1000.0, reset);
        }
        if show_actual_loc {
            out += &format!("{}{:>8}{} ", cyan, stats.actual_loc, reset);
        }
        if show_raw_klocs {
            out += &format!("{}{:>8.3}{} ", green, stats.raw_loc as f64 / 1000.0, reset);
        }
        if show_raw_loc {
            out += &format!("{}{:>8}{} ", green, stats.raw_loc, reset);
        }
        if show_words {
            out += &format!("{}{:>8}{} ", yellow, stats.words, reset);
        }
        if show_chars {
            out += &format!("{}{:>8}{} ", magenta, stats.chars, reset);
        }
        if show_bytes {
            out += &format!("{}{:>8}{} ", blue, stats.bytes, reset);
        }
        if is_sum {
            out += &format!("{}<*> {}{}", cyan, fname, reset);
        } else {
            out += &format!("{}<{}>{} {}", green, lang, reset, fname);
        }
    } else {
        if show_actual_klocs {
            out += &format!("{:>8.3} ", stats.actual_loc as f64 / 1000.0);
        }
        if show_actual_loc {
            out += &format!("{:>8} ", stats.actual_loc);
        }
        if show_raw_klocs {
            out += &format!("{:>8.3} ", stats.raw_loc as f64 / 1000.0);
        }
        if show_raw_loc {
            out += &format!("{:>8} ", stats.raw_loc);
        }
        if show_words {
            out += &format!("{:>8} ", stats.words);
        }
        if show_chars {
            out += &format!("{:>8} ", stats.chars);
        }
        if show_bytes {
            out += &format!("{:>8} ", stats.bytes);
        }
        if is_sum {
            out += &format!("<*> {}", fname);
        } else {
            out += &format!("<{}> {}", lang, fname);
        }
    }

    if filename.is_none() {
        print!("{}", lightgray);
    }
    println!("{}", out.trim_end());
    if filename.is_none() {
        print!("{}", reset);
    }
}

// Help is now handled by clap

fn add_stats(a: Stats, b: Stats) -> Stats {
    Stats {
        actual_loc: a.actual_loc + b.actual_loc,
        raw_loc: a.raw_loc + b.raw_loc,
        words: a.words + b.words,
        chars: a.chars + b.chars,
        bytes: a.bytes + b.bytes,
    }
}

fn is_binary_file(path: &Path) -> bool {
    // Read first 8KB to check for binary content
    const SAMPLE_SIZE: usize = 8192;
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false, // If we can't open it, assume it's not binary
    };
    let mut buffer = vec![0u8; SAMPLE_SIZE];
    match file.read(&mut buffer) {
        Ok(n) => {
            // Check for null bytes in the sample
            buffer[..n].contains(&0)
        }
        Err(_) => false, // If we can't read it, assume it's not binary
    }
}

fn process_file(path: &Path) -> Stats {
    let mut stats = Stats::default();
    
    // Skip binary files
    if is_binary_file(path) {
        return stats;
    }
    
    let lang = detect_language(path);
    let comment_syntax = detect_comment_syntax(&lang, path);
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return stats,
    };
    let mut reader = io::BufReader::new(file);
    let mut buf = String::new();
    let mut in_block_comment = false;
    while let Ok(n) = reader.read_line(&mut buf) {
        if n == 0 {
            break;
        }
        stats.raw_loc += 1;
        stats.bytes += buf.as_bytes().len();
        stats.chars += buf.chars().count();
        stats.words += buf.split_whitespace().count();
        let trimmed = buf.trim();
        let is_empty = trimmed.is_empty();
        let is_comment = is_pure_comment(trimmed, &comment_syntax, &mut in_block_comment);
        if !is_empty && !is_comment {
            stats.actual_loc += 1;
        }
        buf.clear();
    }
    stats
}

fn is_pure_comment(line: &str, syntax: &CommentSyntax, in_block_comment: &mut bool) -> bool {
    if *in_block_comment {
        if let Some(ref end) = syntax.block_end {
            if line.contains(end) {
                *in_block_comment = false;
            }
        }
        return true;
    }
    if let Some(ref start) = syntax.block_start {
        if line.starts_with(start) {
            *in_block_comment = true;
            return true;
        }
    }
    if let Some(ref line_comment) = syntax.line {
        if line.starts_with(line_comment) {
            return true;
        }
    }
    false
}
