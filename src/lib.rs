use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;


#[derive(Debug, Clone)]
pub struct CommentSyntax {
    pub line: Option<String>,
    pub block_start: Option<String>,
    pub block_end: Option<String>,
}

pub fn detect_language(path: &Path) -> String {
    // Try shebang first
    if let Ok(file) = File::open(path) {
        let mut reader = io::BufReader::new(file);
        let mut first_line = String::new();
        if reader.read_line(&mut first_line).is_ok() && first_line.starts_with("#!") {
            if first_line.contains("python") { return "python".to_string(); }
            if first_line.contains("perl") { return "perl".to_string(); }
            if first_line.contains("ruby") { return "ruby".to_string(); }
            if first_line.contains("bash") { return "shell".to_string(); }
            if first_line.contains("sh") { return "shell".to_string(); }
            if first_line.contains("zsh") { return "shell".to_string(); }
            if first_line.contains("node") { return "javascript".to_string(); }
            if first_line.contains("php") { return "php".to_string(); }
            if first_line.contains("lua") { return "lua".to_string(); }
            if first_line.contains("awk") { return "awk".to_string(); }
            if first_line.contains("tcl") { return "tcl".to_string(); }
        }
    }
    // Fallback to extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext {
            "rs" => "rust",
            "c" | "h" => "c",
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "cpp",
            "py" | "python" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "java" => "java",
            "sh" | "bash" | "zsh" | "env" => "shell",
            "css" | "scss" => "css",
            "html" | "htm" => "html",
            "xml" | "xsl" | "xslt" | "xsd" | "dtd" | "xq" => "xml",
            "php" => "php",
            "pl" | "pm" => "perl",
            "go" => "go",
            "scala" => "scala",
            "kt" | "kts" => "kotlin",
            "sql" => "sql",
            "bat" => "batch",
            "bas" | "cls" | "ctl" | "frm" => "vb",
            "jsp" => "jsp",
            "vala" => "vala",
            "sty" => "tex",
            "tcl" => "tcl",
            "txt" => "text",
            "yaml" | "yml" => "yaml",
            "conf" | "ini" => "config",
            _ => ext,
        }.to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn detect_comment_syntax(lang: &str, path: &Path) -> CommentSyntax {
    // Use language name for mapping
    match lang {
        "rust" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "c" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "cpp" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "python" => CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        "shell" => CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        "perl" => CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        "javascript" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "typescript" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "java" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "css" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "html" => CommentSyntax {
            line: None,
            block_start: Some("<!--".into()),
            block_end: Some("-->".into()),
        },
        "xml" => CommentSyntax {
            line: None,
            block_start: Some("<!--".into()),
            block_end: Some("-->".into()),
        },
        "php" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "go" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "scala" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "kotlin" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "sql" => CommentSyntax {
            line: Some("--".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "batch" => CommentSyntax {
            line: Some("REM".into()),
            block_start: None,
            block_end: None,
        },
        "vb" => CommentSyntax {
            line: Some("'".into()),
            block_start: None,
            block_end: None,
        },
        "jsp" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "vala" => CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        "tex" => CommentSyntax {
            line: Some("%".into()),
            block_start: None,
            block_end: None,
        },
        "tcl" => CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        "yaml" => CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        "config" => CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        "text" => CommentSyntax {
            line: None,
            block_start: None,
            block_end: None,
        },
        _ => infer_comment_syntax_from_content(path),
    }
}

pub fn infer_comment_syntax_from_content(path: &Path) -> CommentSyntax {
    // List of candidate comment syntaxes to check
    let candidates = vec![
        CommentSyntax {
            line: Some("//".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        CommentSyntax {
            line: Some("#".into()),
            block_start: None,
            block_end: None,
        },
        CommentSyntax {
            line: Some("--".into()),
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        CommentSyntax {
            line: None,
            block_start: Some("<!--".into()),
            block_end: Some("-->".into()),
        },
        CommentSyntax {
            line: None,
            block_start: Some("/*".into()),
            block_end: Some("*/".into()),
        },
        CommentSyntax {
            line: Some("%".into()),
            block_start: None,
            block_end: None,
        },
        CommentSyntax {
            line: Some("!".into()),
            block_start: None,
            block_end: None,
        },
        CommentSyntax {
            line: Some("REM".into()),
            block_start: None,
            block_end: None,
        },
        CommentSyntax {
            line: Some("'".into()),
            block_start: None,
            block_end: None,
        },
    ];
    let mut counts = vec![0; candidates.len()];
    if let Ok(file) = File::open(path) {
        let reader = io::BufReader::new(file);
        let mut in_block = vec![false; candidates.len()];
        for line in reader.lines().flatten() {
            let l = line.trim();
            for (i, cand) in candidates.iter().enumerate() {
                let mut is_comment = false;
                if in_block[i] {
                    if let Some(ref end) = cand.block_end {
                        if l.contains(end) {
                            in_block[i] = false;
                        }
                    }
                    is_comment = true;
                } else if let Some(ref start) = cand.block_start {
                    if l.starts_with(start) {
                        in_block[i] = true;
                        is_comment = true;
                    }
                } else if let Some(ref line_comment) = cand.line {
                    if l.starts_with(line_comment) {
                        is_comment = true;
                    }
                }
                if is_comment {
                    counts[i] += 1;
                }
            }
        }
    }
    // Pick the candidate with the most matches
    if let Some((idx, _)) = counts.iter().enumerate().max_by_key(|&(_, c)| c) {
        if counts[idx] > 0 {
            return candidates[idx].clone();
        }
    }
    CommentSyntax {
        line: None,
        block_start: None,
        block_end: None,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_detect_language_py() {
        let path = Path::new("foo.py");
        assert_eq!(detect_language(path), "python");
    }

    #[test]
    fn test_detect_language_c() {
        let path = Path::new("foo.c");
        assert_eq!(detect_language(path), "c");
    }

    #[test]
    fn test_detect_language_shebang() {
        use std::fs::File;
        use std::io::Write;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut file = File::create(tmp.path()).unwrap();
        writeln!(file, "#!/usr/bin/env python").unwrap();
        assert_eq!(detect_language(tmp.path()), "python");
    }
}
