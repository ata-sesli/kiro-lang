use std::path::{Path, PathBuf};

use crate::errors::KiroError;
use crate::{grammar, removed_print_statement, unsupported_let_statement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Word,
    String,
    Symbol,
}

#[derive(Debug, Clone)]
struct Token {
    text: String,
    kind: TokenKind,
}

pub fn format_source(source: &str) -> Result<String, KiroError> {
    format_source_for_file(source, "<source>")
}

pub fn format_source_for_file(source: &str, file: &str) -> Result<String, KiroError> {
    if let Some(found) = unsupported_let_statement(source) {
        return Err(KiroError::unsupported_keyword_with_source(
            file,
            source,
            found.line,
            found.column,
            "let",
        ));
    }
    if let Some(removed) = removed_print_statement(source) {
        return Err(KiroError::removed_print_statement(
            file,
            source,
            removed.line,
            removed.column,
        ));
    }
    grammar::parse(source).map_err(|e| KiroError::parse_failed_with_source(file, source, &e))?;
    Ok(format_lines(source))
}

pub fn collect_kiro_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, KiroError> {
    let roots = if paths.is_empty() {
        vec![std::env::current_dir().map_err(|e| {
            KiroError::new(
                crate::errors::ErrorCode::FileNotFound,
                crate::errors::ErrorPhase::Cli,
                format!("Failed to read current directory: {}", e),
            )
        })?]
    } else {
        paths.to_vec()
    };

    let mut files = Vec::new();
    for path in roots {
        collect_path(&path, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), KiroError> {
    if !path.exists() {
        return Err(KiroError::file_not_found(&path.display().to_string()));
    }
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "kiro") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if should_skip_dir(path) {
        return Ok(());
    }

    let entries = std::fs::read_dir(path).map_err(|e| {
        KiroError::new(
            crate::errors::ErrorCode::FileNotFound,
            crate::errors::ErrorPhase::Cli,
            format!("Failed to read '{}': {}", path.display(), e),
        )
        .with_file(path.display().to_string())
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            KiroError::new(
                crate::errors::ErrorCode::FileNotFound,
                crate::errors::ErrorPhase::Cli,
                format!("Failed to read directory entry: {}", e),
            )
        })?;
        collect_path(&entry.path(), files)?;
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name == ".git" || name == "target" || name == ".kiro" || name.starts_with('.')
}

fn format_lines(source: &str) -> String {
    let mut output: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut pending_blank = false;
    let mut previous_top_level_item_end = false;

    for raw_line in source.lines() {
        let trimmed_line = raw_line.trim();
        if trimmed_line.is_empty() {
            pending_blank = true;
            continue;
        }

        let (code_part, comment_part) = split_comment(trimmed_line);
        let normalized_code = normalize_code(code_part.trim());
        let leading_closes = leading_closing_delimiters(&normalized_code);
        let line_depth = depth.saturating_sub(leading_closes);
        let is_comment_only = normalized_code.is_empty();
        let is_doc_comment =
            comment_part.is_some_and(|comment| comment.trim_start().starts_with("///"));
        let is_top_level_item_start = line_depth == 0
            && ((!is_comment_only && starts_top_level_item(&normalized_code))
                || (is_comment_only && is_doc_comment));

        if !output.is_empty() {
            if pending_blank {
                push_blank_once(&mut output);
            } else if previous_top_level_item_end && is_top_level_item_start {
                push_blank_once(&mut output);
            }
        }
        pending_blank = false;

        let indent = " ".repeat(line_depth * 4);
        let line = join_code_and_comment(&normalized_code, comment_part);
        output.push(format!("{}{}", indent, line));

        depth = line_depth + delimiter_delta(&normalized_code);
        previous_top_level_item_end = !is_comment_only && line_depth == 0 && normalized_code == "}";
    }

    while output.last().is_some_and(|line| line.is_empty()) {
        output.pop();
    }

    let mut formatted = output.join("\n");
    formatted.push('\n');
    formatted
}

fn push_blank_once(output: &mut Vec<String>) {
    if output.last().is_some_and(|line| !line.is_empty()) {
        output.push(String::new());
    }
}

fn starts_top_level_item(code: &str) -> bool {
    code.starts_with("import ")
        || code.starts_with("fn ")
        || code.starts_with("pure fn ")
        || code.starts_with("rust fn ")
        || code.starts_with("handle ")
        || code.starts_with("struct ")
        || code.starts_with("error ")
        || code.starts_with("///")
}

fn split_comment(line: &str) -> (&str, Option<&str>) {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '/' && line[idx..].starts_with("//") {
            return (&line[..idx], Some(&line[idx..]));
        }
    }
    (line, None)
}

fn join_code_and_comment(code: &str, comment: Option<&str>) -> String {
    match (code.is_empty(), comment) {
        (true, Some(comment)) => comment.trim().to_string(),
        (false, Some(comment)) => format!("{} {}", code.trim_end(), comment.trim()),
        (_, None) => code.to_string(),
    }
}

fn normalize_code(code: &str) -> String {
    let tokens = tokenize(code);
    render_tokens(&tokens)
}

fn tokenize(code: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = code.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        if ch == '"' {
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < chars.len() {
                let current = chars[i];
                i += 1;
                if escaped {
                    escaped = false;
                } else if current == '\\' {
                    escaped = true;
                } else if current == '"' {
                    break;
                }
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::String,
            });
            continue;
        }

        if ch == '-'
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit()
            && is_unary_position(tokens.last())
        {
            let start = i;
            i += 2;
            let mut seen_decimal = false;
            while i < chars.len() {
                if chars[i].is_ascii_digit() {
                    i += 1;
                } else if chars[i] == '.'
                    && !seen_decimal
                    && i + 1 < chars.len()
                    && chars[i + 1] != '.'
                {
                    seen_decimal = true;
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::Word,
            });
            continue;
        }

        if ch.is_alphanumeric() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::Word,
            });
            continue;
        }

        if i + 1 < chars.len() {
            let pair = [chars[i], chars[i + 1]];
            let pair_text: String = pair.iter().collect();
            if matches!(pair_text.as_str(), "==" | "!=" | ">=" | "<=" | "->" | "..") {
                tokens.push(Token {
                    text: pair_text,
                    kind: TokenKind::Symbol,
                });
                i += 2;
                continue;
            }
        }

        tokens.push(Token {
            text: ch.to_string(),
            kind: TokenKind::Symbol,
        });
        i += 1;
    }

    tokens
}

fn is_unary_position(previous: Option<&Token>) -> bool {
    previous.is_none_or(|token| {
        matches!(
            token.text.as_str(),
            "(" | "{" | "[" | "," | ":" | "=" | "==" | "!=" | ">" | "<" | ">=" | "<=" | "->"
        )
    })
}

fn render_tokens(tokens: &[Token]) -> String {
    let mut out = String::new();
    for (idx, token) in tokens.iter().enumerate() {
        let prev = idx.checked_sub(1).and_then(|i| tokens.get(i));
        let next = tokens.get(idx + 1);
        let text = token.text.as_str();

        match text {
            "," => {
                trim_trailing_spaces(&mut out);
                out.push_str(", ");
            }
            ":" => {
                trim_trailing_spaces(&mut out);
                out.push_str(": ");
            }
            "->" => {
                ensure_space(&mut out);
                out.push_str("-> ");
            }
            ".." | "." => {
                trim_trailing_spaces(&mut out);
                out.push_str(text);
            }
            "!" => {
                trim_trailing_spaces(&mut out);
                out.push('!');
            }
            "=" | "+" | "-" | "*" | "/" | "==" | "!=" | ">" | "<" | ">=" | "<=" => {
                ensure_space(&mut out);
                out.push_str(text);
                out.push(' ');
            }
            "(" | "[" => {
                if text == "(" && prev.is_some_and(|token| token.text == "on") {
                    ensure_space(&mut out);
                } else {
                    trim_trailing_spaces(&mut out);
                }
                out.push_str(text);
            }
            ")" | "]" => {
                trim_trailing_spaces(&mut out);
                out.push_str(text);
            }
            "{" => {
                if needs_space_before_open_brace(prev) {
                    ensure_space(&mut out);
                } else {
                    trim_trailing_spaces(&mut out);
                }
                out.push('{');
                if has_closing_brace_after(tokens, idx) && next.is_some_and(|t| t.text != "}") {
                    out.push(' ');
                }
            }
            "}" => {
                if has_opening_brace_before(tokens, idx) && prev.is_some_and(|t| t.text != "{") {
                    ensure_space(&mut out);
                } else {
                    trim_trailing_spaces(&mut out);
                }
                out.push('}');
            }
            _ => {
                if needs_word_space(prev, token) {
                    ensure_space(&mut out);
                }
                out.push_str(text);
            }
        }
    }
    out.trim_end().to_string()
}

fn needs_space_before_open_brace(previous: Option<&Token>) -> bool {
    previous.is_some_and(|token| {
        !matches!(
            token.text.as_str(),
            "(" | "[" | "{" | "=" | ":" | "," | "->"
        )
    })
}

fn needs_word_space(previous: Option<&Token>, current: &Token) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(previous.text.as_str(), "(" | "[" | "{" | "." | "->") {
        return false;
    }
    if previous.text == ".." {
        return false;
    }
    if matches!(
        current.text.as_str(),
        ")" | "]" | "}" | "," | ":" | "." | ".."
    ) {
        return false;
    }
    previous.kind != TokenKind::Symbol || current.kind != TokenKind::Symbol
}

fn has_closing_brace_after(tokens: &[Token], idx: usize) -> bool {
    tokens[idx + 1..].iter().any(|token| token.text == "}")
}

fn has_opening_brace_before(tokens: &[Token], idx: usize) -> bool {
    tokens[..idx].iter().any(|token| token.text == "{")
}

fn ensure_space(out: &mut String) {
    trim_trailing_spaces(out);
    if !out.is_empty() {
        out.push(' ');
    }
}

fn trim_trailing_spaces(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

fn leading_closing_delimiters(code: &str) -> usize {
    code.chars()
        .take_while(|ch| matches!(ch, '}' | ']' | ')'))
        .count()
}

fn delimiter_delta(code: &str) -> usize {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for ch in code.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        match ch {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}
