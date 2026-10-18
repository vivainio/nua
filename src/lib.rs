//! Bidirectional indentation syntax for Lua/Luau.
mod lua;
use full_moon::{
    ast,
    node::Node,
    tokenizer::{TokenReference, TokenType},
    visitors::Visitor,
};
pub use lua::{strip_types, to_lua};
use std::collections::HashMap;

fn pos(t: &TokenReference) -> usize {
    t.token().start_position().bytes()
}
#[derive(Default)]
struct Change {
    text: Option<String>,
    close: bool,
    open: bool,
    empty: bool,
    statement: bool,
}
#[derive(Default)]
struct Reverse {
    edits: HashMap<usize, Change>,
}
impl Reverse {
    fn text(&mut self, t: &TokenReference, s: &str) {
        self.edits.entry(pos(t)).or_default().text = Some(s.into());
    }
    fn open(&mut self, t: &TokenReference, b: &ast::Block) {
        let e = self.edits.entry(pos(t)).or_default();
        e.open = true;
        e.empty = b.tokens().next().is_none();
    }
    fn close(&mut self, t: &TokenReference, remove: bool) {
        self.edits.entry(pos(t)).or_default().close = true;
        if remove {
            self.text(t, "");
        }
    }
}
impl Visitor for Reverse {
    fn visit_block(&mut self, n: &ast::Block) {
        for stmt in n.stmts() {
            if let Some(t) = stmt.tokens().min_by_key(|t| pos(t)) {
                self.edits.entry(pos(t)).or_default().statement = true;
            }
        }
        if let Some(stmt) = n.last_stmt()
            && let Some(t) = stmt.tokens().min_by_key(|t| pos(t))
        {
            self.edits.entry(pos(t)).or_default().statement = true;
        }
    }
    fn visit_function_args(&mut self, n: &ast::FunctionArgs) {
        if let ast::FunctionArgs::TableConstructor(t) = n {
            self.text(t.braces().tokens().0, "{");
            self.text(t.braces().tokens().1, "}");
        }
    }
    fn visit_table_constructor(&mut self, n: &ast::TableConstructor) {
        if n.fields().iter().all(|f| matches!(f, ast::Field::NoKey(_)))
            && !self.edits.contains_key(&pos(n.braces().tokens().0))
        {
            self.text(n.braces().tokens().0, "[");
            self.text(n.braces().tokens().1, "]");
        }
    }
    fn visit_local_assignment(&mut self, n: &ast::LocalAssignment) {
        self.text(n.local_token(), "let");
    }
    fn visit_local_function(&mut self, n: &ast::LocalFunction) {
        self.text(n.local_token(), "");
        self.text(n.function_token(), "def");
    }
    fn visit_function_declaration(&mut self, n: &ast::FunctionDeclaration) {
        self.text(n.function_token(), "global def");
    }
    fn visit_anonymous_function(&mut self, n: &ast::AnonymousFunction) {
        self.text(n.function_token(), "def");
    }
    fn visit_function_body(&mut self, n: &ast::FunctionBody) {
        let last = if let Some(r) = n.return_type() {
            self.text(r.punctuation(), "->");
            r.type_info().tokens().max_by_key(|t| pos(t)).unwrap()
        } else {
            n.parameters_parentheses().tokens().1
        };
        self.open(last, n.block());
        self.close(n.end_token(), true);
    }
    fn visit_if(&mut self, n: &ast::If) {
        self.text(n.then_token(), "");
        let last = n.condition().tokens().max_by_key(|t| pos(t)).unwrap();
        self.open(last, n.block());
        if let Some(t) = n.else_token() {
            self.close(t, false);
            self.open(t, n.else_block().unwrap());
        }
        self.close(n.end_token(), true);
    }
    fn visit_else_if(&mut self, n: &ast::ElseIf) {
        self.close(n.else_if_token(), false);
        self.text(n.else_if_token(), "elif");
        self.text(n.then_token(), "");
        self.open(
            n.condition().tokens().max_by_key(|t| pos(t)).unwrap(),
            n.block(),
        );
    }
    fn visit_else_if_expression(&mut self, n: &ast::luau::ElseIfExpression) {
        self.text(n.else_if_token(), "elif");
    }
    fn visit_while(&mut self, n: &ast::While) {
        self.text(n.do_token(), "");
        self.open(
            n.condition().tokens().max_by_key(|t| pos(t)).unwrap(),
            n.block(),
        );
        self.close(n.end_token(), true);
    }
    fn visit_numeric_for(&mut self, n: &ast::NumericFor) {
        self.text(n.do_token(), ":");
        self.open(n.do_token(), n.block());
        self.close(n.end_token(), true);
    }
    fn visit_generic_for(&mut self, n: &ast::GenericFor) {
        self.text(n.do_token(), ":");
        self.open(n.do_token(), n.block());
        self.close(n.end_token(), true);
    }
    fn visit_do(&mut self, n: &ast::Do) {
        self.open(n.do_token(), n.block());
        self.close(n.end_token(), true);
    }
    fn visit_repeat(&mut self, n: &ast::Repeat) {
        self.open(n.repeat_token(), n.block());
        self.close(n.until_token(), false);
    }
}

fn parse(s: &str) -> Result<ast::Ast, String> {
    full_moon::parse(s).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Convert Lua/Luau to nua using structural AST boundaries. Formatting is normalized.
pub fn from_luau(source: &str) -> Result<String, String> {
    let tree = parse(source)?;
    let mut reverse = Reverse::default();
    reverse.visit_ast(&tree);
    let mut out = String::new();
    let mut depth = 0usize;
    fn newline(out: &mut String) {
        while out.ends_with(' ') {
            out.pop();
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
    }
    fn emit(out: &mut String, text: &str, depth: usize) {
        if text.is_empty() {
            return;
        }
        if out.is_empty() || out.ends_with('\n') {
            out.push_str(&"  ".repeat(depth));
        }
        out.push_str(text);
    }
    let mut refs: Vec<_> = tree
        .nodes()
        .tokens()
        .chain(std::iter::once(tree.eof()))
        .collect();
    refs.sort_by_key(|t| pos(t));
    refs.dedup_by_key(|t| pos(t));
    for t in refs {
        for trivia in t.leading_trivia() {
            let s = trivia.to_string();
            if s.trim().is_empty() {
                if s.contains('\n') {
                    newline(&mut out);
                } else if !out.ends_with([' ', '\n']) && !out.is_empty() {
                    out.push(' ');
                }
            } else {
                emit(&mut out, &s, depth);
                if s.starts_with("--") && !s.starts_with("--[") {
                    newline(&mut out);
                }
            }
        }
        let edit = reverse.edits.get(&pos(t));
        if edit.is_some_and(|e| e.statement) {
            newline(&mut out);
        }
        if edit.is_some_and(|e| e.close) {
            depth = depth.checked_sub(1).ok_or("invalid block nesting")?;
            newline(&mut out);
        }
        let original = t.token().to_string();
        let escaped;
        let text = if edit.and_then(|e| e.text.as_deref()).is_none()
            && matches!(t.token().token_type(), TokenType::Identifier { .. })
            && matches!(
                original.as_str(),
                "let" | "def" | "global" | "pass" | "elif"
            ) {
            escaped = format!("@{original}");
            &escaped
        } else {
            edit.and_then(|e| e.text.as_deref()).unwrap_or(&original)
        };
        if text == "[" && out.ends_with('[') {
            out.push(' ');
        }
        if text == ":" {
            while out.ends_with(' ') {
                out.pop();
            }
        }
        if text == "->" && !out.ends_with([' ', '\n']) {
            out.push(' ');
        }
        emit(&mut out, text, depth);
        if edit.is_some_and(|e| e.open) {
            if !text.ends_with(':') {
                out.push(':');
            }
            depth += 1;
            // Defer the newline until after header comments.
        }
        for trivia in t.trailing_trivia() {
            let s = trivia.to_string();
            if s.trim().is_empty() {
                if s.contains('\n') {
                    newline(&mut out);
                } else if !out.ends_with([' ', '\n']) && !out.is_empty() {
                    out.push(' ');
                }
            } else {
                emit(&mut out, &s, depth);
                if s.starts_with("--") && !s.starts_with("--[") {
                    newline(&mut out);
                }
            }
        }
        if edit.is_some_and(|e| e.open) {
            newline(&mut out);
            if edit.is_some_and(|e| e.empty) {
                emit(&mut out, "pass", depth);
                newline(&mut out);
            }
        }
    }
    newline(&mut out);
    let back =
        to_luau(&out).map_err(|e| format!("cannot represent this source in nua yet: {e}"))?;
    if !tree.nodes().similar(parse(&back)?.nodes()) {
        return Err(
            "conversion would change the syntax tree; this construct is not supported yet".into(),
        );
    }
    Ok(out)
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
        if line[indent..].starts_with('\t') {
            return Err(format!("line {num}: use spaces for indentation"));
        }
        let code = m.trim();
        if code.is_empty() {
            out.push_str(line);
            out.push('\n');
            continue;
        }
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
                    out.push_str("end\n");
                }
            }
            if let Some(frame) = stack.last_mut() {
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
            text = format!("{}-- pass", " ".repeat(indent));
        } else {
            edits.sort_by_key(|e| e.0);
            for (a, b, s) in edits.into_iter().rev() {
                text.replace_range(a..b, &s);
            }
        }
        out.push_str(&text);
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
