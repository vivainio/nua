//! Bidirectional indentation syntax for Lua/Luau.
mod lua;
mod reverse;
use full_moon::ast;
pub use lua::{strip_types, to_lua};
pub use reverse::from_luau;

fn parse(s: &str) -> Result<ast::Ast, String> {
    full_moon::parse(s).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

// Mask lexical literals and comments, retaining byte offsets and newlines.
fn mask(source: &str) -> Result<String, String> {
    let b = source.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let comment = b[i..].starts_with(b"--");
        if comment {
            i += 2;
        }
        let long_start = i;
        let mut j = i;
        if b.get(j) == Some(&b'[') {
            j += 1;
            while b.get(j) == Some(&b'=') {
                j += 1;
            }
        }
        if j > i && b.get(j) == Some(&b'[') {
            let close = format!("]{}]", "=".repeat(j - i - 1));
            i = j + 1;
            let rest = &source[i..];
            i += rest
                .find(&close)
                .ok_or("unterminated long string/comment")?
                + close.len();
        } else if comment {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if matches!(b[i], b'\'' | b'"' | b'`') {
            let quote = b[i];
            i += 1;
            let mut closed = false;
            while i < b.len() {
                if b[i] == b'\\' {
                    i = (i + 2).min(b.len());
                } else if b[i] == quote {
                    i += 1;
                    closed = true;
                    break;
                } else {
                    i += 1;
                }
            }
            if !closed {
                return Err("unterminated string".into());
            }
        } else {
            i = long_start + 1;
            continue;
        }
        for c in &mut out[start..i] {
            if *c != b'\n' && *c != b'\r' {
                *c = b' ';
            }
        }
        if !comment {
            out[start] = b'0';
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}

// Join multiline headers for analysis while preserving their literal bytes.
fn logical_lines(source: &str, masked: &str) -> Vec<(usize, String, String)> {
    let mut result = Vec::new();
    let mut lines = source.lines().zip(masked.lines()).enumerate().peekable();
    while let Some((index, (line, mask))) = lines.next() {
        let mut line = line.to_string();
        let mut mask = mask.to_string();
        let header = mask.trim_start().starts_with("def ")
            || mask.contains("def(")
            || mask.contains("def (")
            || mask.contains(" def ")
            || ["if ", "elif ", "while ", "for "]
                .iter()
                .any(|s| mask.trim_start().starts_with(s));
        let balance = |s: &str| {
            s.bytes().fold(0i32, |n, c| {
                n + match c {
                    b'(' | b'[' | b'{' => 1,
                    b')' | b']' | b'}' => -1,
                    _ => 0,
                }
            })
        };
        while header && !mask.trim_end().ends_with(':') && balance(&mask) > 0 {
            let Some((_, (next, next_mask))) = lines.next() else {
                break;
            };
            line.push('\n');
            line.push_str(next);
            mask.push('\n');
            mask.push_str(next_mask);
        }
        result.push((index + 1, line, mask));
    }
    result
}

#[derive(Clone)]
struct Frame {
    indent: usize,
    body: Option<usize>,
    kind: String,
    delimiter_base: i32,
}

/// Convert indentation-based nua to validated Luau.
pub fn to_luau(source: &str) -> Result<String, String> {
    let masked = mask(source)?;
    let mut out = String::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut delimiter_depth = 0i32;
    for (num, line, m) in logical_lines(source, &masked) {
        let line = line.as_str();
        let m = m.as_str();
        let indent = line.len() - line.trim_start_matches(' ').len();
        let code = m.trim();
        if code.is_empty() {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if line[indent..].starts_with('\t') {
            return Err(format!("line {num}: use spaces for indentation"));
        }
        let mut join_closing_function = false;
        let continuation = delimiter_depth > stack.last().map_or(0, |f| f.delimiter_base);
        if !continuation {
            let branch = code == "else:" || code.starts_with("elif ");
            let until = code.starts_with("until ");
            while stack.last().is_some_and(|f| indent <= f.indent) {
                let frame = stack.pop().unwrap();
                if frame.body.is_none() {
                    return Err(format!(
                        "line {num}: expected an indented body (use pass for an empty block)"
                    ));
                }
                if frame.indent == indent && branch && matches!(frame.kind.as_str(), "if" | "elif")
                {
                    break;
                }
                if frame.kind == "repeat" {
                    if !until || frame.indent != indent {
                        return Err(format!("line {num}: repeat requires until"));
                    }
                } else {
                    out.push_str(&" ".repeat(frame.indent));
                    out.push_str("end");
                    if frame.kind == "def" && code.starts_with(')') {
                        join_closing_function = true;
                    } else {
                        out.push('\n');
                    }
                }
            }
            if delimiter_depth > stack.last().map_or(0, |f| f.delimiter_base) {
                // Closing a callback resumes the containing table or argument list.
            } else if let Some(frame) = stack.last_mut() {
                match frame.body {
                    None => frame.body = Some(indent),
                    Some(n) if n != indent => {
                        return Err(format!("line {num}: inconsistent indentation"));
                    }
                    _ => {}
                }
            } else if indent != 0 {
                return Err(format!("line {num}: unexpected indentation"));
            }
        }
        let mut text = line.to_string();
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let bytes = m.as_bytes();
        let mut i = 0;
        let mut def_at = None;
        while i < bytes.len() {
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &m[start..i];
                if start > 0
                    && bytes[start - 1] == b'@'
                    && matches!(word, "let" | "def" | "global" | "pass" | "elif")
                {
                    edits.push((start - 1, start, String::new()));
                    continue;
                }
                if word == "let" && start == indent {
                    edits.push((start, i, "local".into()));
                }
                if word == "elif" {
                    edits.push((start, i, "elseif".into()));
                }
                if word == "def" {
                    def_at = Some((start, i));
                }
            } else {
                i += 1;
            }
        }
        let block = code.ends_with(':');
        let kind = code
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches(':')
            .to_string();
        let header_kind = matches!(
            kind.as_str(),
            "if" | "elif" | "else" | "for" | "while" | "do" | "repeat"
        ) || def_at.is_some();
        if block && header_kind {
            let colon = m.rfind(':').unwrap();
            let suffix = match kind.as_str() {
                "if" | "elif" => " then",
                "for" | "while" => " do",
                _ => "",
            };
            edits.push((colon, colon + 1, suffix.into()));
            if let Some((start, end)) = def_at {
                let global = code.starts_with("global def ");
                let after = m[end..colon].trim_start();
                let named = !after.starts_with('(') && !after.starts_with('<');
                let qualified = after
                    .split(['(', '<'])
                    .next()
                    .unwrap_or("")
                    .contains(['.', ':']);
                let replacement = if start == indent && named && !qualified {
                    "local function"
                } else {
                    "function"
                };
                edits.push((start, end, replacement.into()));
                if global {
                    edits.push((indent, start, String::new()));
                }
                // The first arrow outside parentheses and generics is the return annotation.
                let mut level = 0i32;
                let mut generic = 0i32;
                let mut p = end;
                while p + 1 < colon {
                    match bytes[p] {
                        b'(' | b'[' | b'{' => level += 1,
                        b')' | b']' | b'}' => level -= 1,
                        b'<' if level == 0 => generic += 1,
                        b'>' if level == 0 && generic > 0 => generic -= 1,
                        _ => {}
                    }
                    if level == 0 && generic == 0 && &bytes[p..p + 2] == b"->" {
                        edits.push((p, p + 2, ":".into()));
                        break;
                    }
                    p += 1;
                }
            }
            let delta = code.bytes().fold(0, |n, c| {
                n + match c {
                    b'(' | b'[' | b'{' => 1,
                    b')' | b']' | b'}' => -1,
                    _ => 0,
                }
            });
            stack.push(Frame {
                indent,
                body: None,
                delimiter_base: delimiter_depth + delta,
                kind: if def_at.is_some() { "def".into() } else { kind },
            });
        } else if header_kind && def_at.is_some() {
            return Err(format!("line {num}: def header must end with ':'"));
        }
        if code == "pass" {
            text.replace_range(indent..indent + 4, "");
            if text.trim().is_empty() {
                continue;
            }
        } else {
            edits.sort_by_key(|e| e.0);
            for (a, b, s) in edits.into_iter().rev() {
                text.replace_range(a..b, &s);
            }
        }
        out.push_str(if join_closing_function {
            text.trim_start()
        } else {
            &text
        });
        out.push('\n');
        for c in code.bytes() {
            match c {
                b'(' | b'[' | b'{' => delimiter_depth += 1,
                b')' | b']' | b'}' => delimiter_depth -= 1,
                _ => {}
            }
        }
    }
    while let Some(frame) = stack.pop() {
        if frame.body.is_none() {
            return Err("expected an indented body at end of input".into());
        }
        if frame.kind == "repeat" {
            return Err("repeat requires until".into());
        }
        out.push_str(&" ".repeat(frame.indent));
        out.push_str("end\n");
    }
    let out = arrays_to_tables(&out)?;
    parse(&out)?;
    Ok(out)
}

// An opening bracket in expression position is a list; postfix brackets are indexing.
// Bracketed table keys and type indexers are recognized by their following = or :.
fn arrays_to_tables(source: &str) -> Result<String, String> {
    let masked = mask(source)?;
    let b = masked.as_bytes();
    let mut out = source.as_bytes().to_vec();
    let mut brackets = Vec::new();
    let mut expression = true;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            c if c.is_ascii_whitespace() => {
                i += 1;
                continue;
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                i += 1;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                expression = matches!(
                    &masked[start..i],
                    "return"
                        | "in"
                        | "and"
                        | "or"
                        | "not"
                        | "then"
                        | "else"
                        | "until"
                        | "if"
                        | "elseif"
                        | "while"
                );
                continue;
            }
            b'[' => {
                let mut depth = 1;
                let mut end = i + 1;
                while end < b.len() && depth > 0 {
                    match b[end] {
                        b'[' => depth += 1,
                        b']' => depth -= 1,
                        _ => {}
                    }
                    end += 1;
                }
                let next = masked[end..].trim_start();
                let key = (next.starts_with('=') && !next.starts_with("=="))
                    || (next.starts_with(':') && !next.starts_with("::"));
                let list = expression && !key;
                if list {
                    out[i] = b'{';
                }
                brackets.push(list);
                expression = true;
            }
            b']' => {
                if brackets.pop().unwrap_or(false) {
                    out[i] = b'}';
                }
                expression = false;
            }
            b')' | b'}' => expression = false,
            b'0'..=b'9' => expression = false,
            b'.' => {
                if b.get(i + 1) == Some(&b'.') {
                    i += 1;
                    expression = true;
                } else {
                    expression = false;
                }
            }
            _ => expression = true,
        }
        i += 1;
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}
