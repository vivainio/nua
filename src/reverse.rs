//! Structural Lua/Luau reader with layout-preserving rendering.
use super::{parse, to_luau};
use full_moon::{
    ast,
    node::Node,
    tokenizer::{TokenReference, TokenType},
    visitors::{Visitor, VisitorMut},
};
use std::collections::{HashMap, HashSet};

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
    anonymous: Vec<(usize, usize)>,
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
        for (stmt, semicolon) in n.stmts_with_semicolon() {
            if matches!(
                stmt,
                ast::Stmt::If(_)
                    | ast::Stmt::While(_)
                    | ast::Stmt::Do(_)
                    | ast::Stmt::NumericFor(_)
                    | ast::Stmt::GenericFor(_)
                    | ast::Stmt::FunctionDeclaration(_)
                    | ast::Stmt::LocalFunction(_)
            ) && let Some(semicolon) = semicolon
            {
                self.text(semicolon, "");
            }
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
        if !n.fields().is_empty()
            && n.fields().iter().all(|f| matches!(f, ast::Field::NoKey(_)))
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
        let qualified = n
            .name()
            .tokens()
            .any(|t| matches!(t.token().to_string().as_str(), "." | ":"));
        self.text(
            n.function_token(),
            if qualified { "def" } else { "global def" },
        );
    }
    fn visit_anonymous_function(&mut self, n: &ast::AnonymousFunction) {
        self.text(n.function_token(), "def");
        self.anonymous
            .push((pos(n.function_token()), pos(n.body().end_token())));
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

#[derive(Clone, Copy)]
struct Delimiter {
    multiline: bool,
    expand: bool,
}
struct Context {
    id: Option<usize>,
    indent: usize,
    contributes: bool,
    source_indent: usize,
}
#[derive(Default)]
struct Layout {
    out: String,
    contexts: Vec<Context>,
    pending_lines: usize,
    minimum_lines: usize,
    pending_spaces: String,
    line_indent: usize,
    body: Option<bool>,
    next_indent: Option<usize>,
    source_indent: usize,
}
impl Layout {
    fn indent(&self) -> usize {
        self.contexts
            .iter()
            .rev()
            .find(|c| c.contributes)
            .map_or(0, |c| c.indent)
    }
    fn lines(&self) -> usize {
        self.pending_lines.max(self.minimum_lines)
    }
    fn newline(&mut self, count: usize) {
        self.minimum_lines = self.minimum_lines.max(count);
        self.pending_spaces.clear();
    }
    fn whitespace(&mut self, text: &str) {
        let count = text.bytes().filter(|&c| c == b'\n').count();
        if count > 0 {
            self.pending_lines += count;
            self.pending_spaces.clear();
        } else if self.lines() == 0 {
            self.pending_spaces.push_str(text);
        }
    }
    fn emit(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let next_indent = self.next_indent.take();
        if self.lines() > 0 || self.out.is_empty() {
            if !self.out.is_empty() {
                let existing = self.out.bytes().rev().take_while(|&c| c == b'\n').count();
                self.out
                    .push_str(&"\n".repeat(self.lines().saturating_sub(existing)));
            }
            self.line_indent = next_indent.unwrap_or_else(|| self.indent());
            self.out.push_str(&"  ".repeat(self.line_indent));
        } else {
            self.out.push_str(&self.pending_spaces);
        }
        self.pending_lines = 0;
        self.minimum_lines = 0;
        self.pending_spaces.clear();
        self.out.push_str(text);
    }
    fn trivia(&mut self, token: &full_moon::tokenizer::Token, source_indents: &[usize]) {
        let text = token.to_string();
        if matches!(token.token_type(), TokenType::Whitespace { .. }) {
            self.whitespace(&text);
        } else {
            // A section comment may precede the end emitted at the next
            // dedented statement. Honor its original header-level alignment.
            if self.lines() > 0 || self.out.is_empty() {
                let indent = source_indents[token.start_position().line() - 1];
                if self.body == Some(true)
                    && self
                        .contexts
                        .iter()
                        .rev()
                        .find(|c| c.id.is_none())
                        .is_some_and(|c| indent <= c.source_indent)
                {
                    self.before_code();
                }
                if let Some(context) = self
                    .contexts
                    .iter()
                    .rev()
                    .find(|c| c.contributes && c.source_indent == indent)
                {
                    self.next_indent = Some(context.indent - 1);
                } else if indent == 0 {
                    self.next_indent = Some(0);
                }
            }
            if self.lines() == 0 && !self.out.is_empty() && self.pending_spaces.is_empty() {
                self.pending_spaces.push(' ');
            }
            self.emit(&text);
            if matches!(
                token.token_type(),
                TokenType::SingleLineComment { .. } | TokenType::Shebang { .. }
            ) {
                self.newline(1);
            }
        }
    }
    fn before_code(&mut self) {
        if let Some(empty) = self.body.take() {
            self.newline(1);
            if empty {
                let extra = self.pending_lines.saturating_sub(1);
                self.pending_lines = self.pending_lines.min(1);
                self.emit("pass");
                self.pending_lines = extra + 1;
                self.newline(1);
            }
        }
    }
    fn open_block(&mut self, empty: bool) {
        self.contexts.push(Context {
            id: None,
            indent: self.line_indent + 1,
            contributes: true,
            source_indent: self.source_indent,
        });
        self.body = Some(empty);
    }
    fn close_block(&mut self) -> Result<(), String> {
        self.before_code();
        let index = self
            .contexts
            .iter()
            .rposition(|c| c.id.is_none())
            .ok_or("invalid block nesting")?;
        self.contexts.remove(index);
        self.newline(1);
        Ok(())
    }
    fn open_delimiter(&mut self, id: usize, config: Delimiter) {
        self.contexts.push(Context {
            id: Some(id),
            indent: self.line_indent + 1,
            contributes: config.multiline,
            source_indent: self.source_indent,
        });
        if config.expand {
            self.newline(1);
        }
    }
    fn close_delimiter(&mut self, id: usize, expand: bool) {
        if let Some(index) = self.contexts.iter().rposition(|c| c.id == Some(id)) {
            self.next_indent = Some(self.contexts[index].indent - 1);
            self.contexts.remove(index);
        }
        if expand {
            self.newline(1);
        }
    }
    fn finish(mut self) -> String {
        if !self.out.is_empty() {
            self.newline(1);
            self.out.push_str(&"\n".repeat(self.lines()));
        }
        self.out
    }
}

// Semicolons and parentheses around a function literal carry no runtime meaning.
// Normalize only these, retaining all other parentheses (notably multiple returns).
struct Comparable;
impl VisitorMut for Comparable {
    fn visit_block(&mut self, n: ast::Block) -> ast::Block {
        let statements = n.stmts().cloned().map(|s| (s, None)).collect();
        let last = n.last_stmt().cloned().map(|s| (s, None));
        n.with_stmts(statements).with_last_stmt(last)
    }
    fn visit_expression(&mut self, n: ast::Expression) -> ast::Expression {
        match n {
            ast::Expression::Parentheses { expression, .. }
                if matches!(*expression, ast::Expression::Function(_)) =>
            {
                *expression
            }
            _ => n,
        }
    }
}

/// Convert Lua/Luau to nua, retaining blank lines and structured expression layout.
pub fn from_luau(source: &str) -> Result<String, String> {
    let tree = parse(source)?;
    let mut reverse = Reverse::default();
    reverse.visit_ast(&tree);
    let mut refs: Vec<_> = tree
        .nodes()
        .tokens()
        .chain(std::iter::once(tree.eof()))
        .collect();
    refs.sort_by_key(|t| pos(t));
    refs.dedup_by_key(|t| pos(t));
    let mut openings = HashMap::new();
    let mut closings = HashMap::new();
    let mut stack = Vec::new();
    let mut block_positions: Vec<_> = reverse
        .edits
        .iter()
        .filter_map(|(&p, e)| e.open.then_some(p))
        .collect();
    block_positions.sort_unstable();
    for &t in &refs {
        if !matches!(t.token().token_type(), TokenType::Symbol { .. }) {
            continue;
        }
        let text = t.token().to_string();
        if matches!(text.as_str(), "(" | "[" | "{") {
            stack.push(t);
        } else if matches!(text.as_str(), ")" | "]" | "}")
            && let Some(open) = stack.pop()
        {
            let start = pos(open);
            let end = pos(t);
            let index = block_positions.partition_point(|&p| p <= start);
            let body_inside = block_positions.get(index).is_some_and(|&p| p < end);
            let originally_multiline =
                open.token().start_position().line() != t.token().end_position().line();
            openings.insert(
                start,
                Delimiter {
                    multiline: originally_multiline || body_inside,
                    expand: body_inside
                        && (open.token().to_string() == "{" || !originally_multiline),
                },
            );
            closings.insert(end, start);
        }
    }
    let mut wrapped = HashSet::new();
    let mut wrapped_ends = HashMap::new();
    for &(start, end) in &reverse.anonymous {
        let index = refs.partition_point(|t| pos(t) <= end);
        if refs
            .get(index)
            .is_some_and(|t| matches!(t.token().to_string().as_str(), "," | ";" | "}"))
        {
            wrapped.insert(start);
            wrapped_ends.insert(end, start);
        }
    }
    // A retained suffix still needs its physical line: `end)` becomes `)`.
    // Do not consume the preceding line break along with the removed keyword.
    let mut retained_lines: HashMap<usize, usize> = HashMap::new();
    for &t in &refs {
        let original = t.token().to_string();
        let text = reverse
            .edits
            .get(&pos(t))
            .and_then(|e| e.text.as_deref())
            .unwrap_or(&original);
        if !text.is_empty() || wrapped_ends.contains_key(&pos(t)) {
            for (line, offset) in [
                (t.token().start_position().line(), pos(t)),
                (
                    t.token().end_position().line(),
                    t.token().end_position().bytes().saturating_sub(1),
                ),
            ] {
                retained_lines
                    .entry(line)
                    .and_modify(|p| *p = (*p).max(offset))
                    .or_insert(offset);
            }
        }
        for trivia in t.leading_trivia().chain(t.trailing_trivia()) {
            if !matches!(trivia.token_type(), TokenType::Whitespace { .. }) {
                for (line, offset) in [
                    (
                        trivia.start_position().line(),
                        trivia.start_position().bytes(),
                    ),
                    (
                        trivia.end_position().line(),
                        trivia.end_position().bytes().saturating_sub(1),
                    ),
                ] {
                    retained_lines
                        .entry(line)
                        .and_modify(|p| *p = (*p).max(offset))
                        .or_insert(offset);
                }
            }
        }
    }
    let source_indents: Vec<_> = source
        .lines()
        .chain(std::iter::once(""))
        .map(|line| {
            line.bytes()
                .take_while(|c| matches!(c, b' ' | b'\t'))
                .map(|c| if c == b'\t' { 4 } else { 1 })
                .sum()
        })
        .collect();
    let mut layout = Layout::default();
    let mut delimiter_expansion = Vec::new();
    for t in refs {
        layout.source_indent = source_indents[t.token().start_position().line().saturating_sub(1)];
        for trivia in t.leading_trivia() {
            layout.trivia(trivia, &source_indents);
        }
        let p = pos(t);
        let edit = reverse.edits.get(&p);
        if edit.is_some_and(|e| e.statement) {
            layout.newline(1);
        }
        let closed_empty = edit.is_some_and(|e| e.close) && layout.body == Some(true);
        if edit.is_some_and(|e| e.close) {
            layout.close_block()?;
        }
        if let Some(&start) = wrapped_ends.get(&p) {
            layout.close_delimiter(start, true);
        }
        if let Some(&start) = closings.get(&p) {
            layout.close_delimiter(start, openings[&start].expand);
            delimiter_expansion.pop();
        }
        let original = t.token().to_string();
        let mut text = edit
            .and_then(|e| e.text.as_deref())
            .unwrap_or(&original)
            .to_string();
        if edit.and_then(|e| e.text.as_deref()).is_none()
            && matches!(t.token().token_type(), TokenType::Identifier { .. })
            && matches!(
                original.as_str(),
                "let" | "def" | "global" | "pass" | "elif"
            )
        {
            text.insert(0, '@');
        }
        if wrapped.contains(&p) {
            text.insert(0, '(');
        }
        if wrapped_ends.contains_key(&p) {
            text = ")".into();
        }
        if text.is_empty() && matches!(original.as_str(), "end" | "then" | "do") {
            if closed_empty
                || !retained_lines
                    .get(&t.token().start_position().line())
                    .is_some_and(|&offset| offset > p)
            {
                layout.pending_lines = layout.pending_lines.saturating_sub(1);
            }
            layout.pending_spaces.clear();
        }
        if !text.is_empty() {
            layout.before_code();
            if matches!(text.as_str(), ":" | "," | ";") {
                layout.pending_spaces.clear();
            }
            if text == "->" && layout.lines() == 0 && layout.pending_spaces.is_empty() {
                layout.pending_spaces.push(' ');
            }
            if text == "["
                && layout.lines() == 0
                && layout.pending_spaces.is_empty()
                && layout.out.ends_with('[')
            {
                layout.pending_spaces.push(' ');
            }
            if layout.out.ends_with("->") && layout.lines() == 0 && layout.pending_spaces.is_empty()
            {
                layout.pending_spaces.push(' ');
            }
            layout.emit(&text);
        }
        if let Some(&config) = openings.get(&p) {
            layout.open_delimiter(p, config);
            delimiter_expansion.push(config.expand);
        }
        if wrapped.contains(&p) {
            layout.open_delimiter(
                p,
                Delimiter {
                    multiline: true,
                    expand: false,
                },
            );
        }
        if edit.is_some_and(|e| e.open) {
            if !text.ends_with(':') {
                layout.out.push(':');
            }
            layout.open_block(edit.is_some_and(|e| e.empty));
        }
        if matches!(original.as_str(), "," | ";")
            && !text.is_empty()
            && layout.contexts.last().is_some_and(|c| c.id.is_some())
            && delimiter_expansion.last().copied().unwrap_or(false)
        {
            layout.newline(1);
        }
        for trivia in t.trailing_trivia() {
            layout.trivia(trivia, &source_indents);
        }
    }
    let out = layout.finish();
    let back =
        to_luau(&out).map_err(|e| format!("cannot represent this source in nua yet: {e}"))?;
    if !Comparable
        .visit_ast(tree)
        .nodes()
        .similar(Comparable.visit_ast(parse(&back)?).nodes())
    {
        return Err(
            "conversion would change the syntax tree; this construct is not supported yet".into(),
        );
    }
    Ok(out)
}
