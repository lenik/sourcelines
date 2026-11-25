use std::fs::{self, File};
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use clap::{ArgGroup, Parser};
use globset::{Glob, GlobMatcher, GlobSet, GlobSetBuilder};
use sourcelines::{CommentSyntax, detect_comment_syntax, detect_language};

#[derive(Default, Debug, Clone)]
struct Stats {
    actual_loc: usize,
    raw_loc: usize,
    words: usize,
    chars: usize,
    bytes: usize,
}

impl Stats {
    fn add_assign(&mut self, other: &Stats) {
        self.actual_loc += other.actual_loc;
        self.raw_loc += other.raw_loc;
        self.words += other.words;
        self.chars += other.chars;
        self.bytes += other.bytes;
    }
}

/// Precompiled ignore rule. Patterns are compiled once when the ignore file is
/// loaded; matching must never re-parse/recompile globs per path.
#[derive(Clone)]
struct IgnorePattern {
    matcher: GlobMatcher,
    is_negation: bool, // Patterns starting with ! negate previous ignores
    is_dir_only: bool, // Patterns ending with / match only directories
}

impl std::fmt::Debug for IgnorePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IgnorePattern")
            .field("is_negation", &self.is_negation)
            .field("is_dir_only", &self.is_dir_only)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct DirObject {
    path: PathBuf,
    ignore_patterns: Vec<IgnorePattern>,
    parent: Option<Rc<DirObject>>,
}

impl DirObject {
    fn new(path: PathBuf, parent: Option<Rc<DirObject>>) -> Self {
        DirObject {
            path,
            ignore_patterns: Vec::new(),
            parent,
        }
    }

    fn load_ignore_file(&mut self, ignore_file_name: &str, loglevel: u8) {
        let ignore_path = self.path.join(ignore_file_name);
        log_verbose(loglevel, &format!("Loading ignore file: {:?}", ignore_path));
        if let Ok(content) = fs::read_to_string(&ignore_path) {
            log_debug(loglevel, &format!("Successfully read ignore file: {:?}", ignore_path));
            let mut pattern_count = 0;
            for line in content.lines() {
                let line = line.trim();
                // Skip empty lines and comments
                if line.is_empty() || line.starts_with('#') {
                    log_trace(loglevel, &format!("Skipping empty/comment line: '{}'", line));
                    continue;
                }

                let is_negation = line.starts_with('!');
                let pattern_str = if is_negation { &line[1..] } else { line };
                let pattern_str = pattern_str.trim();
                if pattern_str.is_empty() {
                    log_trace(loglevel, "Skipping empty pattern after negation");
                    continue;
                }

                let is_dir_only = pattern_str.ends_with('/');
                let pattern = if is_dir_only {
                    &pattern_str[..pattern_str.len() - 1]
                } else {
                    pattern_str
                };

                log_trace(
                    loglevel,
                    &format!(
                        "Parsed pattern: '{}' (negation: {}, dir_only: {})",
                        pattern, is_negation, is_dir_only
                    ),
                );
                if let Some(matcher) = compile_ignore_glob(pattern) {
                    self.ignore_patterns.push(IgnorePattern {
                        matcher,
                        is_negation,
                        is_dir_only,
                    });
                    pattern_count += 1;
                } else {
                    log_debug(loglevel, &format!("Skipping uncompilable ignore pattern: '{}'", pattern));
                }
            }
            log_verbose(
                loglevel,
                &format!("Loaded {} patterns from {:?}", pattern_count, ignore_path),
            );
        } else {
            log_debug(loglevel, &format!("No ignore file found at: {:?}", ignore_path));
        }
    }

    fn include_test(&self, file_path: &Path, is_dir: bool) -> bool {
        // Get relative path from this directory
        let rel_path = match file_path.strip_prefix(&self.path) {
            Ok(p) => p,
            Err(_) => return true, // If we can't get relative path, include it
        };

        // Convert to string for pattern matching (avoid alloc when already /-separated)
        let path_loss = rel_path.to_string_lossy();
        let path_str: std::borrow::Cow<'_, str> = if path_loss.as_bytes().contains(&b'\\') {
            std::borrow::Cow::Owned(path_loss.replace('\\', "/"))
        } else {
            path_loss
        };

        // Check patterns in order (last match wins)
        let mut included = true;
        let mut matched = false;
        for pattern in &self.ignore_patterns {
            if pattern.is_dir_only && !is_dir {
                continue;
            }

            if pattern.matcher.is_match(path_str.as_ref()) {
                matched = true;
                included = pattern.is_negation;
            }
        }

        // If not matched in this directory, check parent
        if !matched {
            if let Some(ref parent) = self.parent {
                return parent.include_test(file_path, is_dir);
            }
        }

        included
    }
}

/// Convert a gitignore-style pattern to a compiled glob matcher (once per pattern).
fn compile_ignore_glob(pattern: &str) -> Option<GlobMatcher> {
    if pattern == "*" {
        return Glob::new("**").ok().map(|g| g.compile_matcher());
    }

    let mut glob_pattern = if !pattern.starts_with('/') {
        // Unanchored: can match anywhere in the relative path
        format!("**/{}", pattern)
    } else {
        pattern[1..].to_string()
    };

    // Preserve historical normalization used by matches_pattern
    glob_pattern = glob_pattern.replace("**/", "**");
    glob_pattern = glob_pattern.replace("/**", "**");

    match Glob::new(&glob_pattern) {
        Ok(glob) => Some(glob.compile_matcher()),
        Err(_) => {
            // Fallback for odd patterns: escape and treat as literal with **/
            let escaped = glob_escape(pattern);
            let fallback = if pattern.starts_with('/') {
                escaped
            } else {
                format!("**/{}", escaped)
            };
            Glob::new(&fallback).ok().map(|g| g.compile_matcher())
        }
    }
}

fn glob_escape(s: &str) -> String {
    // Escape all special glob characters
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '*' | '?' | '[' | ']' | '{' | '}' | '!' | '(' | ')' | '|' | '^' | '$' | '+' | '.'
            | '#' => {
                out.push('[');
                out.push(c);
                out.push(']');
            }
            _ => out.push(c),
        }
    }
    out
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

    /// Verbose output: with -s, print all file stats; for directories, print per-language summary. Can be used multiple times for more verbosity.
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    verbose: u8,

    /// Quiet output: decrease verbosity. Can be used multiple times.
    #[arg(short = 'q', long = "quiet", action = clap::ArgAction::Count)]
    quiet: u8,

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
    /// Parse ignore list files (like .gitignore) [default: enabled]
    #[arg(short = 'i', long = "ignorelist", default_value_t = true)]
    ignorelist: bool,
    /// Do not parse ignore list files
    #[arg(short = 'I', long = "no-ignorelist")]
    no_ignorelist: bool,
    /// Output summary in text report format
    #[arg(long = "text")]
    text: bool,
    /// Output summary in HTML report format
    #[arg(long = "html")]
    html: bool,
    /// Output summary in LaTeX report format
    #[arg(long = "latex")]
    latex: bool,
    /// Output summary in PDF report format (requires LaTeX)
    #[arg(long = "pdf")]
    pdf: bool,
    /// Output summary in Markdown report format
    #[arg(long = "markdown")]
    markdown: bool,
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
    // If --text is used, also enable recursive and sum by default
    if cli.files.is_empty() {
        cli.files = vec![".".to_string()];
        cli.recursive = true;
        if cli.verbose == 0 {
            cli.verbose = 1; // match historical `-rv .` default
        }
    }
    if cli.text || cli.html || cli.latex || cli.pdf || cli.markdown {
        cli.recursive = true;
        cli.sum = true;
    }

    // Calculate loglevel: base 1, + verbose count, - quiet count, minimum 0
    // UI "verbose" (per-language breakdown) is enabled by any -v.
    // Diagnostic logs need -vv / -vvv / -vvvv (loglevel 3+).
    let loglevel = 1i32 + cli.verbose as i32 - cli.quiet as i32;
    let loglevel = loglevel.max(0) as u8;
    let show_actual_klocs = cli.actual_klocs;
    let show_actual_loc = cli.actual_loc;
    let show_raw_klocs = cli.raw_klocs;
    let show_raw_loc = cli.raw_loc;
    let mut show_words = cli.words;
    let mut show_chars = cli.chars;
    let mut show_bytes = cli.bytes;
    let recursive = cli.recursive;
    let show_sum = cli.sum;
    let verbose = cli.verbose > 0; // per-language / file listing verbosity
    let color = cli.color;
    let follow_symlinks = cli.follow_symlinks;
    let use_ignorelist = cli.ignorelist && !cli.no_ignorelist;
    let text_mode = cli.text;
    let html_mode = cli.html;
    let latex_mode = cli.latex;
    let pdf_mode = cli.pdf;
    let markdown_mode = cli.markdown;
    let files = &cli.files;

    // Cache for compiled ignore patterns per directory (keyed by ignore file path)
    let mut ignore_cache: std::collections::HashMap<PathBuf, Vec<IgnorePattern>> =
        std::collections::HashMap::new();

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
    // (stats, lang, arg, is_dir, optional per-language map for directories)
    let mut file_stats: Vec<(
        Stats,
        String,
        String,
        bool,
        Option<std::collections::HashMap<String, Stats>>,
    )> = Vec::new();
    for arg in files {
        let path = Path::new(arg);
        // Detect VCS once per argument (reused for the whole walk)
        let vcs = if use_ignorelist {
            detect_vcs(path)
        } else {
            VcsType::None
        };
        let ignore_filename = get_ignore_filename(&vcs);
        log_verbose(
            loglevel,
            &format!(
                "Detected VCS {:?} for {:?}, using ignore file: {}",
                vcs, path, ignore_filename
            ),
        );

        if path.is_dir() {
            let dir_obj = if use_ignorelist {
                let mut dir_obj = DirObject::new(path.to_path_buf(), None);
                dir_obj.load_ignore_file(ignore_filename, loglevel);
                ignore_cache.insert(path.join(ignore_filename), dir_obj.ignore_patterns.clone());
                Some(Rc::new(dir_obj))
            } else {
                None
            };
            let (dir_stats, lang_map) = process_dir_lang_filtered(
                path,
                recursive,
                follow_symlinks,
                &exclude_set,
                include_set.as_ref(),
                dir_obj.as_ref(),
                vcs,
                loglevel,
                &mut ignore_cache,
            );
            sum.add_assign(&dir_stats);
            // Save per-language sums for report modes
            for (lang, stats) in lang_map.iter() {
                per_lang_sum.entry(lang.clone()).or_default().add_assign(stats);
            }
            file_stats.push((
                dir_stats,
                "*".to_string(),
                arg.clone(),
                true,
                Some(lang_map),
            ));
        } else {
            log_debug(
                loglevel,
                &format!(
                    "File {:?} in VCS {:?}, using ignore file: {}",
                    path, vcs, ignore_filename
                ),
            );

            let stats = process_file(path);
            sum.add_assign(&stats);
            let lang = detect_language(path);
            file_stats.push((stats, lang, arg.clone(), false, None));
        }
    }

    if (verbose || !show_sum)
        && !text_mode
        && !html_mode
        && !latex_mode
        && !pdf_mode
        && !markdown_mode
    {
        // Print all file stats — reuse lang_map from the first walk (no second scan)
        for (stats, lang, arg, is_dir, lang_map_opt) in &file_stats {
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
                if let Some(lang_map) = lang_map_opt {
                    // Sort grouped (per-language) results by the first visible column descending
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
    }

    // Print output according to -s and -v, or report format modes
    if pdf_mode {
        print_pdf_report(&sum, &per_lang_sum, show_default, show_actual_klocs, show_actual_loc, show_raw_klocs, show_raw_loc, show_words, show_chars, show_bytes);
    } else if latex_mode {
        print_latex_report(&sum, &per_lang_sum, show_default, show_actual_klocs, show_actual_loc, show_raw_klocs, show_raw_loc, show_words, show_chars, show_bytes);
    } else if html_mode {
        print_html_report(&sum, &per_lang_sum, show_default, show_actual_klocs, show_actual_loc, show_raw_klocs, show_raw_loc, show_words, show_chars, show_bytes);
    } else if markdown_mode {
        print_markdown_report(&sum, &per_lang_sum, show_default, show_actual_klocs, show_actual_loc, show_raw_klocs, show_raw_loc, show_words, show_chars, show_bytes);
    } else if text_mode {
        print_text_report(&sum, &per_lang_sum, show_default, show_actual_klocs, show_actual_loc, show_raw_klocs, show_raw_loc, show_words, show_chars, show_bytes);
    } else if show_sum {
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
        parent_dir_obj: Option<&Rc<DirObject>>,
        parent_vcs: VcsType,
        loglevel: u8,
        ignore_cache: &mut std::collections::HashMap<PathBuf, Vec<IgnorePattern>>,
    ) -> (Stats, std::collections::HashMap<String, Stats>) {
        let mut total = Stats::default();
        let mut lang_map: std::collections::HashMap<String, Stats> =
            std::collections::HashMap::new();

        // Inherit VCS from parent; only re-probe locally for nested repositories
        let vcs = detect_vcs_here(path).unwrap_or(parent_vcs);
        let ignore_filename = get_ignore_filename(&vcs);

        // Create DirObject for this directory if ignorelist is enabled
        let dir_obj = if let Some(parent) = parent_dir_obj {
            let mut dir_obj = DirObject::new(path.to_path_buf(), Some(parent.clone()));

            // Check cache first (use VCS-specific cache key)
            let cache_key = path.join(ignore_filename);
            if let Some(cached_patterns) = ignore_cache.get(&cache_key) {
                log_debug(loglevel, &format!("Using cached ignore patterns for: {:?}", path));
                dir_obj.ignore_patterns = cached_patterns.clone();
            } else {
                dir_obj.load_ignore_file(ignore_filename, loglevel);
                ignore_cache.insert(cache_key, dir_obj.ignore_patterns.clone());
            }

            Some(Rc::new(dir_obj))
        } else {
            None
        };

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

            // Prefer DirEntry::file_type to avoid extra stat syscalls per entry
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let is_symlink = file_type.is_symlink();
            if is_symlink && !follow_symlinks {
                continue;
            }

            let is_dir = if is_symlink {
                p.is_dir()
            } else {
                file_type.is_dir()
            };
            let is_file = if is_symlink {
                p.is_file()
            } else {
                file_type.is_file()
            };

            // Check ignore list if enabled
            if let Some(ref dir_obj) = dir_obj {
                if !dir_obj.include_test(&p, is_dir) {
                    continue;
                }
            }

            if recursive && is_dir {
                let (dir_stats, dir_lang_map) = process_dir_lang_filtered(
                    &p,
                    true,
                    follow_symlinks,
                    exclude_set,
                    include_set,
                    dir_obj.as_ref(),
                    vcs,
                    loglevel,
                    ignore_cache,
                );
                total.add_assign(&dir_stats);
                for (lang, stats) in dir_lang_map {
                    lang_map.entry(lang).or_default().add_assign(&stats);
                }
            } else if is_file {
                let stats = process_file(&p);
                let lang = detect_language(&p);
                lang_map.entry(lang).or_default().add_assign(&stats);
                total.add_assign(&stats);
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

fn print_text_report(
    sum: &Stats,
    per_lang_sum: &std::collections::HashMap<String, Stats>,
    show_default: bool,
    show_actual_klocs: bool,
    show_actual_loc: bool,
    show_raw_klocs: bool,
    show_raw_loc: bool,
    show_words: bool,
    show_chars: bool,
    show_bytes: bool,
) {
    println!("Source Code Statistics Report");
    println!("{}", "=".repeat(80));
    println!();
    
    // Summary section
    println!("Summary:");
    println!("{}", "-".repeat(80));
    if show_actual_klocs || (show_default && show_actual_loc) {
        if show_actual_klocs {
            println!("  Actual Lines of Code (KLOC): {:>12.3}", sum.actual_loc as f64 / 1000.0);
        } else {
            println!("  Actual Lines of Code:        {:>12}", sum.actual_loc);
        }
    }
    if show_raw_klocs || (show_default && show_raw_loc) {
        if show_raw_klocs {
            println!("  Raw Lines of Code (KLOC):    {:>12.3}", sum.raw_loc as f64 / 1000.0);
        } else {
            println!("  Raw Lines of Code:           {:>12}", sum.raw_loc);
        }
    }
    if show_words || show_default {
        println!("  Words:                       {:>12}", sum.words);
    }
    if show_chars || show_default {
        println!("  Characters:                  {:>12}", sum.chars);
    }
    if show_bytes || show_default {
        println!("  Bytes:                       {:>12}", sum.bytes);
    }
    println!();
    
    // Per-language breakdown
    if !per_lang_sum.is_empty() {
        println!("Per-Language Breakdown:");
        println!("{}", "-".repeat(80));
        
        // Sort by actual_loc descending
        let mut lang_items: Vec<(&String, &Stats)> = per_lang_sum.iter().collect();
        lang_items.sort_by(|(_, sa), (_, sb)| sb.actual_loc.cmp(&sa.actual_loc));
        
        // Calculate table width
        let mut table_width = 20; // Language column
        let num_cols = (if show_actual_klocs || (show_default && show_actual_loc) { 1 } else { 0 })
            + (if show_raw_klocs || (show_default && show_raw_loc) { 1 } else { 0 })
            + (if show_words || show_default { 1 } else { 0 })
            + (if show_chars || show_default { 1 } else { 0 })
            + (if show_bytes || show_default { 1 } else { 0 });
        table_width += num_cols * 13; // 12 chars + 1 space for each column
        table_width += 2; // Leading spaces
        
        // Print header
        print!("  {:<20}", "Language");
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                print!(" {:>12}", "Actual KLOC");
            } else {
                print!(" {:>12}", "Actual LOC");
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                print!(" {:>12}", "Raw KLOC");
            } else {
                print!(" {:>12}", "Raw LOC");
            }
        }
        if show_words || show_default {
            print!(" {:>12}", "Words");
        }
        if show_chars || show_default {
            print!(" {:>12}", "Chars");
        }
        if show_bytes || show_default {
            print!(" {:>12}", "Bytes");
        }
        println!();
        println!("  {}", "-".repeat(table_width - 2));
        
        // Print data
        for (lang, stats) in lang_items {
            // Filter out zero-count languages
            if stats.actual_loc == 0 && stats.raw_loc == 0 && stats.words == 0 && stats.chars == 0 && stats.bytes == 0 {
                continue;
            }
            
            print!("  {:<20}", lang);
            if show_actual_klocs || (show_default && show_actual_loc) {
                if show_actual_klocs {
                    print!(" {:>12.3}", stats.actual_loc as f64 / 1000.0);
                } else {
                    print!(" {:>12}", stats.actual_loc);
                }
            }
            if show_raw_klocs || (show_default && show_raw_loc) {
                if show_raw_klocs {
                    print!(" {:>12.3}", stats.raw_loc as f64 / 1000.0);
                } else {
                    print!(" {:>12}", stats.raw_loc);
                }
            }
            if show_words || show_default {
                print!(" {:>12}", stats.words);
            }
            if show_chars || show_default {
                print!(" {:>12}", stats.chars);
            }
            if show_bytes || show_default {
                print!(" {:>12}", stats.bytes);
            }
            println!();
        }
        println!();
    }
    
    println!("{}", "=".repeat(80));
}

fn print_html_report(
    sum: &Stats,
    per_lang_sum: &std::collections::HashMap<String, Stats>,
    show_default: bool,
    show_actual_klocs: bool,
    show_actual_loc: bool,
    show_raw_klocs: bool,
    show_raw_loc: bool,
    show_words: bool,
    show_chars: bool,
    show_bytes: bool,
) {
    println!("<!DOCTYPE html>");
    println!("<html lang=\"en\">");
    println!("<head>");
    println!("  <meta charset=\"UTF-8\">");
    println!("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">");
    println!("  <title>Source Code Statistics Report</title>");
    println!("  <style>");
    println!("    body {{ font-family: Arial, sans-serif; margin: 20px; background-color: #f5f5f5; }}");
    println!("    .container {{ max-width: 1200px; margin: 0 auto; background-color: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}");
    println!("    h1 {{ color: #333; border-bottom: 3px solid #4CAF50; padding-bottom: 10px; }}");
    println!("    h2 {{ color: #555; margin-top: 30px; }}");
    println!("    table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}");
    println!("    th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #ddd; }}");
    println!("    th {{ background-color: #4CAF50; color: white; font-weight: bold; }}");
    println!("    tr:hover {{ background-color: #f5f5f5; }}");
    println!("    .summary {{ background-color: #e8f5e9; padding: 15px; border-radius: 5px; margin: 20px 0; }}");
    println!("    .summary-item {{ margin: 8px 0; font-size: 16px; }}");
    println!("    .summary-label {{ font-weight: bold; color: #2e7d32; }}");
    println!("  </style>");
    println!("</head>");
    println!("<body>");
    println!("  <div class=\"container\">");
    println!("    <h1>Source Code Statistics Report</h1>");
    
    // Summary section
    println!("    <h2>Summary</h2>");
    println!("    <div class=\"summary\">");
    if show_actual_klocs || (show_default && show_actual_loc) {
        if show_actual_klocs {
            println!("      <div class=\"summary-item\"><span class=\"summary-label\">Actual Lines of Code (KLOC):</span> {:.3}</div>", sum.actual_loc as f64 / 1000.0);
        } else {
            println!("      <div class=\"summary-item\"><span class=\"summary-label\">Actual Lines of Code:</span> {}</div>", sum.actual_loc);
        }
    }
    if show_raw_klocs || (show_default && show_raw_loc) {
        if show_raw_klocs {
            println!("      <div class=\"summary-item\"><span class=\"summary-label\">Raw Lines of Code (KLOC):</span> {:.3}</div>", sum.raw_loc as f64 / 1000.0);
        } else {
            println!("      <div class=\"summary-item\"><span class=\"summary-label\">Raw Lines of Code:</span> {}</div>", sum.raw_loc);
        }
    }
    if show_words || show_default {
        println!("      <div class=\"summary-item\"><span class=\"summary-label\">Words:</span> {}</div>", sum.words);
    }
    if show_chars || show_default {
        println!("      <div class=\"summary-item\"><span class=\"summary-label\">Characters:</span> {}</div>", sum.chars);
    }
    if show_bytes || show_default {
        println!("      <div class=\"summary-item\"><span class=\"summary-label\">Bytes:</span> {}</div>", sum.bytes);
    }
    println!("    </div>");
    
    // Per-language breakdown
    if !per_lang_sum.is_empty() {
        println!("    <h2>Per-Language Breakdown</h2>");
        println!("    <table>");
        println!("      <thead>");
        print!("        <tr><th>Language</th>");
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                print!("<th>Actual KLOC</th>");
            } else {
                print!("<th>Actual LOC</th>");
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                print!("<th>Raw KLOC</th>");
            } else {
                print!("<th>Raw LOC</th>");
            }
        }
        if show_words || show_default {
            print!("<th>Words</th>");
        }
        if show_chars || show_default {
            print!("<th>Chars</th>");
        }
        if show_bytes || show_default {
            print!("<th>Bytes</th>");
        }
        println!("</tr>");
        println!("      </thead>");
        println!("      <tbody>");
        
        // Sort by actual_loc descending
        let mut lang_items: Vec<(&String, &Stats)> = per_lang_sum.iter().collect();
        lang_items.sort_by(|(_, sa), (_, sb)| sb.actual_loc.cmp(&sa.actual_loc));
        
        for (lang, stats) in lang_items {
            // Filter out zero-count languages
            if stats.actual_loc == 0 && stats.raw_loc == 0 && stats.words == 0 && stats.chars == 0 && stats.bytes == 0 {
                continue;
            }
            
            print!("        <tr><td>{}</td>", lang);
            if show_actual_klocs || (show_default && show_actual_loc) {
                if show_actual_klocs {
                    print!("<td>{:.3}</td>", stats.actual_loc as f64 / 1000.0);
                } else {
                    print!("<td>{}</td>", stats.actual_loc);
                }
            }
            if show_raw_klocs || (show_default && show_raw_loc) {
                if show_raw_klocs {
                    print!("<td>{:.3}</td>", stats.raw_loc as f64 / 1000.0);
                } else {
                    print!("<td>{}</td>", stats.raw_loc);
                }
            }
            if show_words || show_default {
                print!("<td>{}</td>", stats.words);
            }
            if show_chars || show_default {
                print!("<td>{}</td>", stats.chars);
            }
            if show_bytes || show_default {
                print!("<td>{}</td>", stats.bytes);
            }
            println!("</tr>");
        }
        
        println!("      </tbody>");
        println!("    </table>");
    }
    
    println!("  </div>");
    println!("</body>");
    println!("</html>");
}

fn print_latex_report(
    sum: &Stats,
    per_lang_sum: &std::collections::HashMap<String, Stats>,
    show_default: bool,
    show_actual_klocs: bool,
    show_actual_loc: bool,
    show_raw_klocs: bool,
    show_raw_loc: bool,
    show_words: bool,
    show_chars: bool,
    show_bytes: bool,
) {
    println!("\\documentclass{{article}}");
    println!("\\usepackage[utf8]{{inputenc}}");
    println!("\\usepackage{{booktabs}}");
    println!("\\usepackage{{longtable}}");
    println!("\\usepackage{{geometry}}");
    println!("\\geometry{{a4paper, margin=1in}}");
    println!("\\title{{Source Code Statistics Report}}");
    println!("\\author{{sourcelines}}");
    println!("\\date{{\\today}}");
    println!("\\begin{{document}}");
    println!("\\maketitle");
    println!();
    println!("\\section{{Summary}}");
    println!("\\begin{{itemize}}");
    
    if show_actual_klocs || (show_default && show_actual_loc) {
        if show_actual_klocs {
            println!("  \\item \\textbf{{Actual Lines of Code (KLOC):}} {:.3}", sum.actual_loc as f64 / 1000.0);
        } else {
            println!("  \\item \\textbf{{Actual Lines of Code:}} {}", sum.actual_loc);
        }
    }
    if show_raw_klocs || (show_default && show_raw_loc) {
        if show_raw_klocs {
            println!("  \\item \\textbf{{Raw Lines of Code (KLOC):}} {:.3}", sum.raw_loc as f64 / 1000.0);
        } else {
            println!("  \\item \\textbf{{Raw Lines of Code:}} {}", sum.raw_loc);
        }
    }
    if show_words || show_default {
        println!("  \\item \\textbf{{Words:}} {}", sum.words);
    }
    if show_chars || show_default {
        println!("  \\item \\textbf{{Characters:}} {}", sum.chars);
    }
    if show_bytes || show_default {
        println!("  \\item \\textbf{{Bytes:}} {}", sum.bytes);
    }
    println!("\\end{{itemize}}");
    println!();
    
    // Per-language breakdown
    if !per_lang_sum.is_empty() {
        println!("\\section{{Per-Language Breakdown}}");
        println!("\\begin{{longtable}}{{l");
        if show_actual_klocs || (show_default && show_actual_loc) {
            print!("r");
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            print!("r");
        }
        if show_words || show_default {
            print!("r");
        }
        if show_chars || show_default {
            print!("r");
        }
        if show_bytes || show_default {
            print!("r");
        }
        println!("}}");
        println!("\\toprule");
        print!("  \\textbf{{Language}}");
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                print!(" & \\textbf{{Actual KLOC}}");
            } else {
                print!(" & \\textbf{{Actual LOC}}");
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                print!(" & \\textbf{{Raw KLOC}}");
            } else {
                print!(" & \\textbf{{Raw LOC}}");
            }
        }
        if show_words || show_default {
            print!(" & \\textbf{{Words}}");
        }
        if show_chars || show_default {
            print!(" & \\textbf{{Chars}}");
        }
        if show_bytes || show_default {
            print!(" & \\textbf{{Bytes}}");
        }
        println!(" \\\\");
        println!("\\midrule");
        println!("\\endfirsthead");
        println!("\\multicolumn{{{}}}{{c}}{{\\textit{{Continued from previous page}}}} \\\\", 
                 1 + (if show_actual_klocs || (show_default && show_actual_loc) { 1 } else { 0 })
                 + (if show_raw_klocs || (show_default && show_raw_loc) { 1 } else { 0 })
                 + (if show_words || show_default { 1 } else { 0 })
                 + (if show_chars || show_default { 1 } else { 0 })
                 + (if show_bytes || show_default { 1 } else { 0 }));
        println!("\\toprule");
        print!("  \\textbf{{Language}}");
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                print!(" & \\textbf{{Actual KLOC}}");
            } else {
                print!(" & \\textbf{{Actual LOC}}");
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                print!(" & \\textbf{{Raw KLOC}}");
            } else {
                print!(" & \\textbf{{Raw LOC}}");
            }
        }
        if show_words || show_default {
            print!(" & \\textbf{{Words}}");
        }
        if show_chars || show_default {
            print!(" & \\textbf{{Chars}}");
        }
        if show_bytes || show_default {
            print!(" & \\textbf{{Bytes}}");
        }
        println!(" \\\\");
        println!("\\midrule");
        println!("\\endhead");
        println!("\\bottomrule");
        println!("\\endfoot");
        println!("\\bottomrule");
        println!("\\endlastfoot");
        
        // Sort by actual_loc descending
        let mut lang_items: Vec<(&String, &Stats)> = per_lang_sum.iter().collect();
        lang_items.sort_by(|(_, sa), (_, sb)| sb.actual_loc.cmp(&sa.actual_loc));
        
        for (lang, stats) in lang_items {
            // Filter out zero-count languages
            if stats.actual_loc == 0 && stats.raw_loc == 0 && stats.words == 0 && stats.chars == 0 && stats.bytes == 0 {
                continue;
            }
            
            // Escape LaTeX special characters in language name
            let lang_escaped = lang.replace('&', "\\&").replace('%', "\\%").replace('$', "\\$").replace('#', "\\#").replace('^', "\\textasciicircum{}").replace('_', "\\_").replace('{', "\\{").replace('}', "\\}");
            
            print!("  {}", lang_escaped);
            if show_actual_klocs || (show_default && show_actual_loc) {
                if show_actual_klocs {
                    print!(" & {:.3}", stats.actual_loc as f64 / 1000.0);
                } else {
                    print!(" & {}", stats.actual_loc);
                }
            }
            if show_raw_klocs || (show_default && show_raw_loc) {
                if show_raw_klocs {
                    print!(" & {:.3}", stats.raw_loc as f64 / 1000.0);
                } else {
                    print!(" & {}", stats.raw_loc);
                }
            }
            if show_words || show_default {
                print!(" & {}", stats.words);
            }
            if show_chars || show_default {
                print!(" & {}", stats.chars);
            }
            if show_bytes || show_default {
                print!(" & {}", stats.bytes);
            }
            println!(" \\\\");
        }
        
        println!("\\end{{longtable}}");
    }
    
    println!("\\end{{document}}");
}

fn print_markdown_report(
    sum: &Stats,
    per_lang_sum: &std::collections::HashMap<String, Stats>,
    show_default: bool,
    show_actual_klocs: bool,
    show_actual_loc: bool,
    show_raw_klocs: bool,
    show_raw_loc: bool,
    show_words: bool,
    show_chars: bool,
    show_bytes: bool,
) {
    println!("# Source Code Statistics Report");
    println!();
    
    // Summary section
    println!("## Summary");
    println!();
    if show_actual_klocs || (show_default && show_actual_loc) {
        if show_actual_klocs {
            println!("- **Actual Lines of Code (KLOC):** {:.3}", sum.actual_loc as f64 / 1000.0);
        } else {
            println!("- **Actual Lines of Code:** {}", sum.actual_loc);
        }
    }
    if show_raw_klocs || (show_default && show_raw_loc) {
        if show_raw_klocs {
            println!("- **Raw Lines of Code (KLOC):** {:.3}", sum.raw_loc as f64 / 1000.0);
        } else {
            println!("- **Raw Lines of Code:** {}", sum.raw_loc);
        }
    }
    if show_words || show_default {
        println!("- **Words:** {}", sum.words);
    }
    if show_chars || show_default {
        println!("- **Characters:** {}", sum.chars);
    }
    if show_bytes || show_default {
        println!("- **Bytes:** {}", sum.bytes);
    }
    println!();
    
    // Per-language breakdown
    if !per_lang_sum.is_empty() {
        println!("## Per-Language Breakdown");
        println!();
        
        // Sort by actual_loc descending
        let mut lang_items: Vec<(&String, &Stats)> = per_lang_sum.iter().collect();
        lang_items.sort_by(|(_, sa), (_, sb)| sb.actual_loc.cmp(&sa.actual_loc));
        
        // Print table header
        print!("| Language");
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                print!(" | Actual KLOC");
            } else {
                print!(" | Actual LOC");
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                print!(" | Raw KLOC");
            } else {
                print!(" | Raw LOC");
            }
        }
        if show_words || show_default {
            print!(" | Words");
        }
        if show_chars || show_default {
            print!(" | Chars");
        }
        if show_bytes || show_default {
            print!(" | Bytes");
        }
        println!(" |");
        
        // Print separator
        print!("|");
        let num_cols = 1 + (if show_actual_klocs || (show_default && show_actual_loc) { 1 } else { 0 })
            + (if show_raw_klocs || (show_default && show_raw_loc) { 1 } else { 0 })
            + (if show_words || show_default { 1 } else { 0 })
            + (if show_chars || show_default { 1 } else { 0 })
            + (if show_bytes || show_default { 1 } else { 0 });
        for _ in 0..num_cols {
            print!(" --- |");
        }
        println!();
        
        // Print data rows
        for (lang, stats) in lang_items {
            // Filter out zero-count languages
            if stats.actual_loc == 0 && stats.raw_loc == 0 && stats.words == 0 && stats.chars == 0 && stats.bytes == 0 {
                continue;
            }
            
            print!("| {}", lang);
            if show_actual_klocs || (show_default && show_actual_loc) {
                if show_actual_klocs {
                    print!(" | {:.3}", stats.actual_loc as f64 / 1000.0);
                } else {
                    print!(" | {}", stats.actual_loc);
                }
            }
            if show_raw_klocs || (show_default && show_raw_loc) {
                if show_raw_klocs {
                    print!(" | {:.3}", stats.raw_loc as f64 / 1000.0);
                } else {
                    print!(" | {}", stats.raw_loc);
                }
            }
            if show_words || show_default {
                print!(" | {}", stats.words);
            }
            if show_chars || show_default {
                print!(" | {}", stats.chars);
            }
            if show_bytes || show_default {
                print!(" | {}", stats.bytes);
            }
            println!(" |");
        }
        println!();
    }
}

fn print_pdf_report(
    sum: &Stats,
    per_lang_sum: &std::collections::HashMap<String, Stats>,
    show_default: bool,
    show_actual_klocs: bool,
    show_actual_loc: bool,
    show_raw_klocs: bool,
    show_raw_loc: bool,
    show_words: bool,
    show_chars: bool,
    show_bytes: bool,
) {
    use std::io::Write;
    use std::process::Command;
    
    // Generate LaTeX content
    let mut latex_content = Vec::new();
    let mut latex_writer = std::io::Cursor::new(&mut latex_content);
    
    writeln!(latex_writer, "\\documentclass{{article}}").unwrap();
    writeln!(latex_writer, "\\usepackage[utf8]{{inputenc}}").unwrap();
    writeln!(latex_writer, "\\usepackage{{booktabs}}").unwrap();
    writeln!(latex_writer, "\\usepackage{{longtable}}").unwrap();
    writeln!(latex_writer, "\\usepackage{{geometry}}").unwrap();
    writeln!(latex_writer, "\\geometry{{a4paper, margin=1in}}").unwrap();
    writeln!(latex_writer, "\\title{{Source Code Statistics Report}}").unwrap();
    writeln!(latex_writer, "\\author{{sourcelines}}").unwrap();
    writeln!(latex_writer, "\\date{{\\today}}").unwrap();
    writeln!(latex_writer, "\\begin{{document}}").unwrap();
    writeln!(latex_writer, "\\maketitle").unwrap();
    writeln!(latex_writer).unwrap();
    writeln!(latex_writer, "\\section{{Summary}}").unwrap();
    writeln!(latex_writer, "\\begin{{itemize}}").unwrap();
    
    if show_actual_klocs || (show_default && show_actual_loc) {
        if show_actual_klocs {
            writeln!(latex_writer, "  \\item \\textbf{{Actual Lines of Code (KLOC):}} {:.3}", sum.actual_loc as f64 / 1000.0).unwrap();
        } else {
            writeln!(latex_writer, "  \\item \\textbf{{Actual Lines of Code:}} {}", sum.actual_loc).unwrap();
        }
    }
    if show_raw_klocs || (show_default && show_raw_loc) {
        if show_raw_klocs {
            writeln!(latex_writer, "  \\item \\textbf{{Raw Lines of Code (KLOC):}} {:.3}", sum.raw_loc as f64 / 1000.0).unwrap();
        } else {
            writeln!(latex_writer, "  \\item \\textbf{{Raw Lines of Code:}} {}", sum.raw_loc).unwrap();
        }
    }
    if show_words || show_default {
        writeln!(latex_writer, "  \\item \\textbf{{Words:}} {}", sum.words).unwrap();
    }
    if show_chars || show_default {
        writeln!(latex_writer, "  \\item \\textbf{{Characters:}} {}", sum.chars).unwrap();
    }
    if show_bytes || show_default {
        writeln!(latex_writer, "  \\item \\textbf{{Bytes:}} {}", sum.bytes).unwrap();
    }
    writeln!(latex_writer, "\\end{{itemize}}").unwrap();
    writeln!(latex_writer).unwrap();
    
    // Per-language breakdown
    if !per_lang_sum.is_empty() {
        writeln!(latex_writer, "\\section{{Per-Language Breakdown}}").unwrap();
        let col_spec = format!("l{}{}{}{}{}",
            if show_actual_klocs || (show_default && show_actual_loc) { "r" } else { "" },
            if show_raw_klocs || (show_default && show_raw_loc) { "r" } else { "" },
            if show_words || show_default { "r" } else { "" },
            if show_chars || show_default { "r" } else { "" },
            if show_bytes || show_default { "r" } else { "" });
        writeln!(latex_writer, "\\begin{{longtable}}{{{}}}", col_spec).unwrap();
        writeln!(latex_writer, "\\toprule").unwrap();
        write!(latex_writer, "  \\textbf{{Language}}").unwrap();
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                write!(latex_writer, " & \\textbf{{Actual KLOC}}").unwrap();
            } else {
                write!(latex_writer, " & \\textbf{{Actual LOC}}").unwrap();
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                write!(latex_writer, " & \\textbf{{Raw KLOC}}").unwrap();
            } else {
                write!(latex_writer, " & \\textbf{{Raw LOC}}").unwrap();
            }
        }
        if show_words || show_default {
            write!(latex_writer, " & \\textbf{{Words}}").unwrap();
        }
        if show_chars || show_default {
            write!(latex_writer, " & \\textbf{{Chars}}").unwrap();
        }
        if show_bytes || show_default {
            write!(latex_writer, " & \\textbf{{Bytes}}").unwrap();
        }
        writeln!(latex_writer, " \\\\").unwrap();
        writeln!(latex_writer, "\\midrule").unwrap();
        writeln!(latex_writer, "\\endfirsthead").unwrap();
        writeln!(latex_writer, "\\multicolumn{{{}}}{{c}}{{\\textit{{Continued from previous page}}}} \\\\", 
                 1 + (if show_actual_klocs || (show_default && show_actual_loc) { 1 } else { 0 })
                 + (if show_raw_klocs || (show_default && show_raw_loc) { 1 } else { 0 })
                 + (if show_words || show_default { 1 } else { 0 })
                 + (if show_chars || show_default { 1 } else { 0 })
                 + (if show_bytes || show_default { 1 } else { 0 })).unwrap();
        writeln!(latex_writer, "\\toprule").unwrap();
        write!(latex_writer, "  \\textbf{{Language}}").unwrap();
        if show_actual_klocs || (show_default && show_actual_loc) {
            if show_actual_klocs {
                write!(latex_writer, " & \\textbf{{Actual KLOC}}").unwrap();
            } else {
                write!(latex_writer, " & \\textbf{{Actual LOC}}").unwrap();
            }
        }
        if show_raw_klocs || (show_default && show_raw_loc) {
            if show_raw_klocs {
                write!(latex_writer, " & \\textbf{{Raw KLOC}}").unwrap();
            } else {
                write!(latex_writer, " & \\textbf{{Raw LOC}}").unwrap();
            }
        }
        if show_words || show_default {
            write!(latex_writer, " & \\textbf{{Words}}").unwrap();
        }
        if show_chars || show_default {
            write!(latex_writer, " & \\textbf{{Chars}}").unwrap();
        }
        if show_bytes || show_default {
            write!(latex_writer, " & \\textbf{{Bytes}}").unwrap();
        }
        writeln!(latex_writer, " \\\\").unwrap();
        writeln!(latex_writer, "\\midrule").unwrap();
        writeln!(latex_writer, "\\endhead").unwrap();
        writeln!(latex_writer, "\\bottomrule").unwrap();
        writeln!(latex_writer, "\\endfoot").unwrap();
        writeln!(latex_writer, "\\bottomrule").unwrap();
        writeln!(latex_writer, "\\endlastfoot").unwrap();
        
        // Sort by actual_loc descending
        let mut lang_items: Vec<(&String, &Stats)> = per_lang_sum.iter().collect();
        lang_items.sort_by(|(_, sa), (_, sb)| sb.actual_loc.cmp(&sa.actual_loc));
        
        for (lang, stats) in lang_items {
            // Filter out zero-count languages
            if stats.actual_loc == 0 && stats.raw_loc == 0 && stats.words == 0 && stats.chars == 0 && stats.bytes == 0 {
                continue;
            }
            
            // Escape LaTeX special characters
            let lang_escaped = lang.replace('&', "\\&").replace('%', "\\%").replace('$', "\\$").replace('#', "\\#").replace('^', "\\textasciicircum{}").replace('_', "\\_").replace('{', "\\{").replace('}', "\\}");
            
            write!(latex_writer, "  {}", lang_escaped).unwrap();
            if show_actual_klocs || (show_default && show_actual_loc) {
                if show_actual_klocs {
                    write!(latex_writer, " & {:.3}", stats.actual_loc as f64 / 1000.0).unwrap();
                } else {
                    write!(latex_writer, " & {}", stats.actual_loc).unwrap();
                }
            }
            if show_raw_klocs || (show_default && show_raw_loc) {
                if show_raw_klocs {
                    write!(latex_writer, " & {:.3}", stats.raw_loc as f64 / 1000.0).unwrap();
                } else {
                    write!(latex_writer, " & {}", stats.raw_loc).unwrap();
                }
            }
            if show_words || show_default {
                write!(latex_writer, " & {}", stats.words).unwrap();
            }
            if show_chars || show_default {
                write!(latex_writer, " & {}", stats.chars).unwrap();
            }
            if show_bytes || show_default {
                write!(latex_writer, " & {}", stats.bytes).unwrap();
            }
            writeln!(latex_writer, " \\\\").unwrap();
        }
        
        writeln!(latex_writer, "\\end{{longtable}}").unwrap();
    }
    
    writeln!(latex_writer, "\\end{{document}}").unwrap();
    
    // Write LaTeX to temporary file
    let temp_dir = std::env::temp_dir();
    let temp_tex = temp_dir.join("sourcelines_report.tex");
    fs::write(&temp_tex, &latex_content).unwrap_or_else(|_| {
        eprintln!("Error: Could not write temporary LaTeX file");
        std::process::exit(1);
    });
    
    // Compile LaTeX to PDF using pdflatex
    let output = Command::new("pdflatex")
        .arg("-interaction=nonstopmode")
        .arg("-output-directory")
        .arg(&temp_dir)
        .arg(&temp_tex)
        .output();
    
    match output {
        Ok(result) => {
            if result.status.success() {
                // Read the generated PDF and output it
                let pdf_path = temp_dir.join("sourcelines_report.pdf");
                if let Ok(pdf_data) = fs::read(&pdf_path) {
                    use std::io::{self, Write};
                    let stdout = io::stdout();
                    let mut handle = stdout.lock();
                    handle.write_all(&pdf_data).unwrap_or_else(|_| {
                        eprintln!("Error: Could not write PDF to stdout");
                        std::process::exit(1);
                    });
                } else {
                    eprintln!("Error: PDF file was not generated");
                    std::process::exit(1);
                }
            } else {
                eprintln!("Error: pdflatex failed");
                if !result.stderr.is_empty() {
                    eprintln!("{}", String::from_utf8_lossy(&result.stderr));
                }
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("Error: pdflatex not found. Please install a LaTeX distribution (e.g., texlive)");
            std::process::exit(1);
        }
    }
}

// Help is now handled by clap

fn process_file(path: &Path) -> Stats {
    use std::io::Seek;

    let mut stats = Stats::default();
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return stats,
    };

    // Binary probe on the same handle, then rewind for line counting
    const SAMPLE_SIZE: usize = 8192;
    let mut sample = [0u8; SAMPLE_SIZE];
    let sample_len = match file.read(&mut sample) {
        Ok(n) => n,
        Err(_) => return stats,
    };
    if sample[..sample_len].contains(&0) {
        return stats;
    }
    if file.seek(io::SeekFrom::Start(0)).is_err() {
        return stats;
    }

    let lang = detect_language(path);
    let comment_syntax = detect_comment_syntax(&lang, path);
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

// Logging functions
// Thresholds: -v enables UI verbosity (loglevel 2); diagnostics need -vv and above.
fn log_verbose(loglevel: u8, msg: &str) {
    if loglevel >= 3 {
        eprintln!("[VERBOSE] {}", msg);
    }
}

fn log_debug(loglevel: u8, msg: &str) {
    if loglevel >= 4 {
        eprintln!("[DEBUG] {}", msg);
    }
}

fn log_trace(loglevel: u8, msg: &str) {
    if loglevel >= 5 {
        eprintln!("[TRACE] {}", msg);
    }
}

// Version control detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VcsType {
    Git,
    Svn,
    Hg,
    None,
}

/// Cheap local probe: only look in `path` itself (for nested repos during a walk).
fn detect_vcs_here(path: &Path) -> Option<VcsType> {
    if path.join(".git").is_dir() {
        return Some(VcsType::Git);
    }
    if path.join(".svn").is_dir() {
        return Some(VcsType::Svn);
    }
    if path.join(".hg").is_dir() {
        return Some(VcsType::Hg);
    }
    None
}

fn detect_vcs(path: &Path) -> VcsType {
    let mut current = path;

    // Check each directory in the path hierarchy
    loop {
        if let Some(vcs) = detect_vcs_here(current) {
            return vcs;
        }

        // Move to parent directory
        match current.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                current = parent;
            }
            _ => break,
        }
    }

    VcsType::None
}

fn get_ignore_filename(vcs: &VcsType) -> &'static str {
    match vcs {
        VcsType::Git => ".gitignore",
        VcsType::Svn => ".svnignore",
        VcsType::Hg => ".hgignore",
        VcsType::None => ".gitignore", // Default to gitignore
    }
}
