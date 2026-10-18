//! Erase Luau's compile-time types and validate the result as Lua 5.1.
use full_moon::{
    LuaVersion, ast,
    node::Node,
    tokenizer::{StringLiteralQuoteType, Token, TokenReference, TokenType},
    visitors::Visitor,
};
use std::collections::HashSet;

#[derive(Default)]
struct Eraser {
    removed: HashSet<usize>,
    spans: Vec<(usize, usize)>,
}
impl Eraser {
    fn erase(&mut self, node: &impl Node) {
        let start = node
            .tokens()
            .map(|t| t.token().start_position().bytes())
            .min();
        let end = node
            .tokens()
            .map(|t| t.token().end_position().bytes())
            .max();
        if let (Some(start), Some(end)) = (start, end) {
            self.spans.push((start, end));
        }
        for token in node.tokens() {
            self.removed.insert(token.token().start_position().bytes());
        }
    }
}
impl Visitor for Eraser {
    fn visit_type_specifier(&mut self, n: &ast::luau::TypeSpecifier) {
        self.erase(n);
    }
    fn visit_type_assertion(&mut self, n: &ast::luau::TypeAssertion) {
        self.erase(n);
    }
    fn visit_generic_declaration(&mut self, n: &ast::luau::GenericDeclaration) {
        self.erase(n);
    }
    fn visit_type_instantiation(&mut self, n: &ast::luau::TypeInstantiation) {
        self.erase(n);
    }
    fn visit_type_declaration(&mut self, n: &ast::luau::TypeDeclaration) {
        self.erase(n);
    }
    fn visit_exported_type_declaration(&mut self, n: &ast::luau::ExportedTypeDeclaration) {
        self.erase(n);
    }
    fn visit_type_function(&mut self, n: &ast::luau::TypeFunction) {
        self.erase(n);
    }
    fn visit_exported_type_function(&mut self, n: &ast::luau::ExportedTypeFunction) {
        self.erase(n);
    }
    fn visit_block(&mut self, n: &ast::Block) {
        for (stmt, semicolon) in n.stmts_with_semicolon() {
            if matches!(
                stmt,
                ast::Stmt::TypeDeclaration(_)
                    | ast::Stmt::ExportedTypeDeclaration(_)
                    | ast::Stmt::TypeFunction(_)
                    | ast::Stmt::ExportedTypeFunction(_)
            ) && let Some(semicolon) = semicolon
            {
                self.erase(semicolon);
            }
        }
    }
}

#[derive(Default)]
struct Compatibility {
    errors: Vec<(usize, String)>,
}
impl Compatibility {
    fn reject(&mut self, node: &impl Node, message: &str) {
        if let Some(p) = node.start_position() {
            self.errors
                .push((p.bytes(), format!("line {}: {message}", p.line())));
        }
    }
}
impl Visitor for Compatibility {
    fn visit_generic_for(&mut self, n: &ast::GenericFor) {
        // A bare table and a bare iterator variable are indistinguishable statically.
        // Require a call/function, or an explicit Lua iterator/state/control tuple.
        if n.expressions().iter().count() == 1
            && !matches!(
                n.expressions().iter().next(),
                Some(ast::Expression::FunctionCall(_) | ast::Expression::Function(_))
            )
        {
            self.reject(n, "generalized iteration is not supported by Lua 5.1; use pairs(t), ipairs(t), or an explicit iterator/state tuple");
        }
    }
    fn visit_compound_assignment(&mut self, n: &ast::CompoundAssignment) {
        self.reject(
            n,
            "compound assignment is not supported by Lua 5.1; use an explicit assignment",
        );
    }
    fn visit_if_expression(&mut self, n: &ast::luau::IfExpression) {
        self.reject(
            n,
            "inline conditional expressions are not supported by Lua 5.1; use an if statement",
        );
    }
    fn visit_interpolated_string(&mut self, n: &ast::luau::InterpolatedString) {
        self.reject(
            n,
            "backtick strings are not supported by Lua 5.1; use quoted strings and concatenation",
        );
    }
    fn visit_last_stmt(&mut self, n: &ast::LastStmt) {
        if matches!(n, ast::LastStmt::Continue(_)) {
            self.reject(n, "continue is not supported by Lua 5.1");
        }
    }
    fn visit_number(&mut self, n: &Token) {
        let s = n.to_string();
        if s.contains('_')
            || s.starts_with("0b")
            || s.starts_with("0B")
            || ((s.starts_with("0x") || s.starts_with("0X")) && s.contains(['.', 'p', 'P']))
        {
            self.errors.push((
                n.start_position().bytes(),
                format!(
                    "line {}: this numeric literal is not supported by Lua 5.1",
                    n.start_position().line()
                ),
            ));
        }
    }
    fn visit_string_literal(&mut self, n: &Token) {
        // Lua 5.1 accepts unknown escape letters differently; syntax parsing alone
        // cannot catch \x, \u, and \z changing the resulting string value.
        if let TokenType::StringLiteral {
            literal,
            quote_type,
            ..
        } = n.token_type()
            && *quote_type != StringLiteralQuoteType::Brackets
        {
            let b = literal.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'\\' {
                    if let Some(b'x' | b'u' | b'z') = b.get(i + 1) {
                        self.errors.push((
                            n.start_position().bytes(),
                            format!(
                                "line {}: this string escape is not supported by Lua 5.1",
                                n.start_position().line()
                            ),
                        ));
                        break;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
}

/// Strip compile-time types from existing Luau and return validated Lua 5.1.
/// Luau runtime extensions are rejected, not lowered.
pub fn strip_types(source: &str) -> Result<String, String> {
    let tree = super::parse(source)?;
    let mut eraser = Eraser::default();
    eraser.visit_ast(&tree);
    let mut tokens: Vec<&TokenReference> = tree.nodes().tokens().collect();
    tokens.sort_by_key(|t| t.token().start_position().bytes());
    tokens.dedup_by_key(|t| t.token().start_position().bytes());
    let mut removed = vec![false; source.len()];
    for reference in tokens.iter().copied().chain(std::iter::once(tree.eof())) {
        let token = reference.token();
        if eraser.removed.contains(&token.start_position().bytes()) {
            removed[token.start_position().bytes()..token.end_position().bytes()].fill(true);
        }
        for trivia in reference
            .leading_trivia()
            .chain(reference.trailing_trivia())
        {
            let (a, b) = (
                trivia.start_position().bytes(),
                trivia.end_position().bytes(),
            );
            if matches!(trivia.token_type(), TokenType::Whitespace { .. })
                && eraser
                    .spans
                    .iter()
                    .any(|&(start, end)| a >= start && b <= end)
            {
                removed[a..b].fill(true);
            }
        }
    }
    let bytes: Vec<u8> = source
        .bytes()
        .enumerate()
        .filter_map(|(i, c)| (!removed[i] || matches!(c, b'\n' | b'\r')).then_some(c))
        .collect();
    let out = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    // Clean trailing whitespace only in trivia, never in string/comment tokens.
    let parsed = super::parse(&out)?;
    let mut whitespace = Vec::new();
    for reference in parsed.nodes().tokens().chain(std::iter::once(parsed.eof())) {
        for trivia in reference
            .leading_trivia()
            .chain(reference.trailing_trivia())
        {
            if matches!(trivia.token_type(), TokenType::Whitespace { .. }) {
                let text = trivia.to_string();
                let mut replacement = String::new();
                for part in text.split_inclusive('\n') {
                    if let Some(line) = part.strip_suffix('\n') {
                        replacement.push_str(line.trim_end_matches([' ', '\t']));
                        replacement.push('\n');
                    } else if trivia.end_position().bytes() == out.len() {
                        replacement.push_str(part.trim_end_matches([' ', '\t']));
                    } else {
                        replacement.push_str(part);
                    }
                }
                if text != replacement {
                    whitespace.push((
                        trivia.start_position().bytes(),
                        trivia.end_position().bytes(),
                        replacement,
                    ));
                }
            }
        }
    }
    whitespace.sort_by_key(|e| e.0);
    whitespace.dedup_by_key(|e| e.0);
    let mut out = out;
    for (a, b, replacement) in whitespace.into_iter().rev() {
        out.replace_range(a..b, &replacement);
    }
    // Inspect the erased tree so runtime-like expressions inside typeof/type
    // functions do not cause false rejections.
    let erased =
        super::parse(&out).map_err(|e| format!("type erasure produced invalid syntax: {e}"))?;
    let mut compatibility = Compatibility::default();
    compatibility.visit_ast(&erased);
    if !compatibility.errors.is_empty() {
        compatibility.errors.sort_by_key(|e| e.0);
        return Err(compatibility
            .errors
            .into_iter()
            .map(|e| e.1)
            .collect::<Vec<_>>()
            .join("\n"));
    }
    full_moon::parse_fallible(&out, LuaVersion::lua51())
        .into_result()
        .map_err(|errors| {
            format!(
                "remaining syntax is not supported by Lua 5.1:\n{}",
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })?;
    Ok(out)
}

/// Transpile nua to Lua 5.1 by first emitting Luau, then erasing types.
pub fn to_lua(source: &str) -> Result<String, String> {
    strip_types(&super::to_luau(source)?)
}
