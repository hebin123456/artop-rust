//! `.mtl` template parser (port of C++ `emf-acceleo/AcceleoParser.cpp`,
//! aligned to Java `org.eclipse.acceleo.model.mtpl.util.MtlResourceImpl`).
//!
//! Strategy: scan the source, separating literal text from `[...]` blocks. A
//! `[...]` block is dispatched on its leading token: `template`/`query`/
//! `for`/`if`/`let`/`file`/`protected`/`import`/`extends`/`module`; anything
//! else is an `[expr/]` evaluation block.
//!
//! The parser accepts the same AQL sub-set as the C++ implementation:
//! `self`/variables, `'string'`/`"string"`, integers, `true`/`false`,
//! `expr.name` navigation, `expr->name(args)` collection operations,
//! `expr.name(args)` calls, `cond ? a : b`, and lambdas `e | expr`.

use crate::ast::*;
use std::rc::Rc;

/// An error produced while parsing an MTL source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleoParseError {
    /// Human-readable message.
    pub message: String,
}

impl std::fmt::Display for AcceleoParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AcceleoParseError {}

impl AcceleoParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Parse `.mtl` source text into a [`Module`] AST.
pub fn parse(source: &str) -> Result<Rc<Module>, AcceleoParseError> {
    Parser::new(source).parse_module()
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_part(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    // ===== lexical helpers =====

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self, s: &str) -> bool {
        let b = s.as_bytes();
        if self.pos + b.len() > self.bytes.len() {
            return false;
        }
        &self.bytes[self.pos..self.pos + b.len()] == b
    }

    fn consume(&mut self, s: &str) -> bool {
        if self.peek(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, s: &str, what: &str) -> Result<(), AcceleoParseError> {
        if self.consume(s) {
            Ok(())
        } else {
            Err(AcceleoParseError::new(format!(
                "expected '{s}' {what} at pos {}",
                self.pos
            )))
        }
    }

    /// Consume a single line break directly after a block's opening tag; this
    /// lets a tag occupy its own line without polluting the output (matching
    /// Acceleo/MTL). Only one `\n` (or `\r\n`) is consumed.
    fn skip_line_after_tag(&mut self) {
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\r' {
            self.pos += 1;
            if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\n' {
                self.pos += 1;
            }
        } else if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\n' {
            self.pos += 1;
        }
    }

    fn parse_identifier(&mut self) -> Result<String, AcceleoParseError> {
        self.skip_whitespace();
        let start = self.pos;
        while self.pos < self.bytes.len() && is_ident_part(self.bytes[self.pos]) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(AcceleoParseError::new(format!(
                "expected identifier at pos {}",
                self.pos
            )));
        }
        Ok(self.src[start..self.pos].to_string())
    }

    fn parse_qualified_name(&mut self) -> Result<String, AcceleoParseError> {
        let mut name = self.parse_identifier()?;
        loop {
            self.skip_whitespace();
            if self.pos + 1 < self.bytes.len()
                && self.bytes[self.pos] == b'.'
                && is_ident_start(self.bytes[self.pos + 1])
            {
                self.pos += 1;
                name.push('.');
                name.push_str(&self.parse_identifier()?);
            } else {
                break;
            }
        }
        Ok(name)
    }

    /// Parse a string literal, accepting both `'...'` (AQL) and `"..."`.
    fn parse_string_literal(&mut self) -> Result<String, AcceleoParseError> {
        self.skip_whitespace();
        let quote = if self.pos < self.bytes.len()
            && (self.bytes[self.pos] == b'\'' || self.bytes[self.pos] == b'"')
        {
            let q = self.bytes[self.pos];
            self.pos += 1;
            q
        } else {
            return Err(AcceleoParseError::new(format!(
                "expected string literal at pos {}",
                self.pos
            )));
        };
        let mut out: Vec<u8> = Vec::new();
        while self.pos < self.bytes.len() && self.bytes[self.pos] != quote {
            let c = self.bytes[self.pos];
            self.pos += 1;
            if c == b'\\' && self.pos < self.bytes.len() {
                let e = self.bytes[self.pos];
                self.pos += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'\'' => out.push(b'\''),
                    b'"' => out.push(b'"'),
                    b'\\' => out.push(b'\\'),
                    other => {
                        out.push(b'\\');
                        out.push(other);
                    }
                }
            } else {
                out.push(c);
            }
        }
        if self.pos < self.bytes.len() {
            self.pos += 1; // closing quote
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    fn parse_integer(&mut self) -> Result<i64, AcceleoParseError> {
        self.skip_whitespace();
        let start = self.pos;
        if self.pos < self.bytes.len()
            && (self.bytes[self.pos] == b'-' || self.bytes[self.pos] == b'+')
        {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(AcceleoParseError::new(format!(
                "expected integer at pos {}",
                self.pos
            )));
        }
        self.src[start..self.pos]
            .parse::<i64>()
            .map_err(|_| AcceleoParseError::new(format!("invalid integer at pos {start}")))
    }

    // ===== module =====

    fn parse_module(&mut self) -> Result<Rc<Module>, AcceleoParseError> {
        self.skip_whitespace();
        self.expect("[", "to start module")?;
        if !self.consume("module") {
            return Err(AcceleoParseError::new(format!(
                "expected 'module' after [ at pos {}",
                self.pos
            )));
        }
        let mut m = Module {
            name: String::new(),
            params: Vec::new(),
            imports: Vec::new(),
            extends: Vec::new(),
            templates: Vec::new(),
            queries: Vec::new(),
        };
        self.parse_module_header(&mut m)?;
        self.skip_whitespace();
        self.expect("]", "to end module header")?;
        self.parse_module_body(&mut m)?;
        self.expect("[", "for [/module]")?;
        self.expect("/", "for [/module]")?;
        if !self.consume("module") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/module]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "for [/module]")?;
        Ok(Rc::new(m))
    }

    fn parse_module_header(&mut self, m: &mut Module) -> Result<(), AcceleoParseError> {
        m.name = self.parse_identifier()?;
        self.skip_whitespace();
        if self.consume("(") {
            if !self.peek(")") {
                m.params.push(self.parse_param()?);
                while self.consume(",") {
                    m.params.push(self.parse_param()?);
                }
            }
            self.expect(")", "to close module params")?;
        }
        self.skip_whitespace();
        if self.peek("extends") {
            self.consume("extends");
            m.extends.push(self.parse_qualified_name()?);
            while self.consume(",") {
                m.extends.push(self.parse_qualified_name()?);
            }
        }
        Ok(())
    }

    fn parse_param(&mut self) -> Result<Param, AcceleoParseError> {
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.expect(":", "in parameter declaration")?;
        let type_name = self.parse_qualified_name()?;
        Ok(Param { name, type_name })
    }

    fn parse_module_body(&mut self, m: &mut Module) -> Result<(), AcceleoParseError> {
        loop {
            self.skip_whitespace();
            if self.peek("[/module]") || self.pos >= self.bytes.len() {
                break;
            }
            if !self.consume("[") {
                return Err(AcceleoParseError::new(format!(
                    "expected '[' at pos {}",
                    self.pos
                )));
            }
            if self.consume("import") {
                m.imports.push(self.parse_string_literal()?);
                self.skip_whitespace();
                self.consume("/");
                self.expect("]", "after import")?;
            } else if self.consume("extends") {
                m.extends.push(self.parse_qualified_name()?);
                while self.consume(",") {
                    m.extends.push(self.parse_qualified_name()?);
                }
                self.skip_whitespace();
                self.consume("/");
                self.expect("]", "after extends")?;
            } else if self.consume("template") {
                m.templates.push(self.parse_template()?);
            } else if self.consume("query") {
                m.queries.push(self.parse_query()?);
            } else {
                return Err(AcceleoParseError::new(format!(
                    "unknown directive in module body at pos {}",
                    self.pos
                )));
            }
        }
        Ok(())
    }

    fn parse_template(&mut self) -> Result<Rc<Template>, AcceleoParseError> {
        let mut is_public = true;
        self.skip_whitespace();
        while self.peek("public") || self.peek("private") || self.peek("protected") {
            if self.peek("public") {
                self.consume("public");
                is_public = true;
            } else if self.peek("private") {
                self.consume("private");
                is_public = false;
            } else {
                self.consume("protected");
            }
            self.skip_whitespace();
        }
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        let mut params = Vec::new();
        if self.consume("(") {
            if !self.peek(")") {
                params.push(self.parse_param()?);
                while self.consume(",") {
                    params.push(self.parse_param()?);
                }
            }
            self.expect(")", "to close template params")?;
        }
        // Optional `post(...)` — skipped but tolerated.
        self.skip_whitespace();
        if self.peek("post") {
            self.consume("post");
            self.skip_whitespace();
            if self.consume("(") {
                let mut depth = 1;
                while self.pos < self.bytes.len() && depth > 0 {
                    if self.bytes[self.pos] == b'(' {
                        depth += 1;
                    } else if self.bytes[self.pos] == b')' {
                        depth -= 1;
                    }
                    if depth > 0 {
                        self.pos += 1;
                    }
                }
                if self.pos < self.bytes.len() {
                    self.pos += 1; // closing ')'
                }
            }
        }
        self.expect("]", "to end template header")?;
        self.skip_line_after_tag();

        let body = self.parse_blocks(&["template"])?;

        self.expect("[", "after template body")?;
        self.expect("/", "after template body")?;
        if !self.consume("template") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/template]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "after template body")?;
        Ok(Rc::new(Template {
            name,
            params,
            is_public,
            body,
            post_literal: String::new(),
        }))
    }

    fn parse_query(&mut self) -> Result<Rc<Query>, AcceleoParseError> {
        self.skip_whitespace();
        while self.peek("public") || self.peek("private") || self.peek("protected") {
            if self.peek("public") {
                self.consume("public");
            } else if self.peek("private") {
                self.consume("private");
            } else {
                self.consume("protected");
            }
            self.skip_whitespace();
        }
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        let mut params = Vec::new();
        if self.consume("(") {
            if !self.peek(")") {
                params.push(self.parse_param()?);
                while self.consume(",") {
                    params.push(self.parse_param()?);
                }
            }
            self.expect(")", "to close query params")?;
        }
        self.skip_whitespace();
        self.expect(":", "in query return type")?;
        let return_type_name = self.parse_qualified_name()?;
        self.skip_whitespace();
        self.expect("=", "in query body")?;
        let body = self.parse_expression()?;
        self.skip_whitespace();
        self.consume("/");
        self.expect("]", "after query")?;
        Ok(Rc::new(Query {
            name,
            params,
            return_type_name,
            body,
        }))
    }

    // ===== blocks =====

    fn parse_static_text(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'[' {
            self.pos += 1;
        }
        self.src[start..self.pos].to_string()
    }

    fn parse_blocks(&mut self, _end_tags: &[&str]) -> Result<Vec<Rc<Block>>, AcceleoParseError> {
        let mut blocks: Vec<Rc<Block>> = Vec::new();
        while self.pos < self.bytes.len() {
            if self.peek("[/") {
                return Ok(blocks);
            }
            if self.peek("[elseif") || self.peek("[else") {
                return Ok(blocks);
            }
            if self.peek("[") {
                let b = self.parse_bracket_block()?;
                blocks.push(b);
            } else {
                let txt = self.parse_static_text();
                if !txt.is_empty() {
                    blocks.push(Block::Text(txt).rc());
                }
            }
        }
        Ok(blocks)
    }

    fn parse_bracket_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        let start_pos = self.pos;
        self.expect("[", "for block")?;
        self.skip_whitespace();
        if self.consume("for") {
            return self.parse_for_block();
        }
        if self.consume("if") {
            return self.parse_if_block();
        }
        if self.consume("let") {
            return self.parse_let_block();
        }
        if self.consume("file") {
            return self.parse_file_block();
        }
        if self.consume("protected") {
            return self.parse_protected_block();
        }
        // Otherwise this is an `[expr/]` block; rewind to `[` and parse it.
        self.pos = start_pos;
        self.parse_expr_block()
    }

    fn parse_expr_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        self.expect("[", "for expression block")?;
        let e = self.parse_expression()?;
        self.skip_whitespace();
        self.consume("/");
        self.expect("]", "to close expr block")?;
        Ok(Block::Expr(e).rc())
    }

    fn parse_for_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        self.skip_whitespace();
        self.expect("(", "in for")?;
        let var_name = self.parse_identifier()?;
        self.skip_whitespace();
        let mut var_type_name = String::new();
        if self.consume(":") {
            var_type_name = self.parse_qualified_name()?;
            self.skip_whitespace();
        }
        self.expect("|", "in for")?;
        let collection = self.parse_expression()?;
        self.expect(")", "in for")?;
        let mut has_separator = false;
        let mut separator = String::new();
        self.skip_whitespace();
        if self.consume("sep") {
            has_separator = true;
            self.skip_whitespace();
            self.expect("(", "in for sep")?;
            if self.pos < self.bytes.len()
                && (self.bytes[self.pos] == b'\'' || self.bytes[self.pos] == b'"')
            {
                separator = self.parse_string_literal()?;
            } else {
                let _ = self.parse_expression()?;
            }
            self.expect(")", "in for sep")?;
        }
        self.expect("]", "to end for header")?;
        self.skip_line_after_tag();
        let body = self.parse_blocks(&["for"])?;
        self.expect("[", "after for body")?;
        self.expect("/", "after for body")?;
        if !self.consume("for") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/for]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "after for body")?;
        Ok(Block::For(ForBlock {
            var_name,
            var_type_name,
            collection,
            has_separator,
            separator,
            body,
        })
        .rc())
    }

    fn parse_if_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        self.skip_whitespace();
        self.expect("(", "in if")?;
        let cond = self.parse_expression()?;
        self.expect(")", "in if")?;
        self.expect("]", "to end if header")?;
        self.skip_line_after_tag();

        let then_body = self.parse_blocks(&["if", "elseif", "else"])?;

        let mut else_ifs = Vec::new();
        while self.peek("[elseif") {
            self.expect("[", "for elseif")?;
            self.consume("elseif");
            self.skip_whitespace();
            self.expect("(", "in elseif")?;
            let c = self.parse_expression()?;
            self.expect(")", "in elseif")?;
            self.expect("]", "to end elseif header")?;
            self.skip_line_after_tag();
            let body = self.parse_blocks(&["if", "elseif", "else"])?;
            else_ifs.push((c, body));
        }
        let mut else_body = Vec::new();
        if self.peek("[else") {
            self.expect("[", "for else")?;
            self.consume("else");
            self.expect("]", "to end else header")?;
            self.skip_line_after_tag();
            else_body = self.parse_blocks(&["if"])?;
        }
        self.expect("[", "after if body")?;
        self.expect("/", "after if body")?;
        if !self.consume("if") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/if]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "after if body")?;
        Ok(Block::If(IfBlock {
            cond,
            then_body,
            else_ifs,
            else_body,
        })
        .rc())
    }

    fn parse_let_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        self.skip_whitespace();
        let var_name = self.parse_identifier()?;
        self.skip_whitespace();
        let mut var_type_name = String::new();
        if self.consume(":") {
            var_type_name = self.parse_qualified_name()?;
            self.skip_whitespace();
        }
        self.expect("=", "in let")?;
        let value = self.parse_expression()?;
        self.expect("]", "to end let header")?;
        self.skip_line_after_tag();
        let body = self.parse_blocks(&["let"])?;
        self.expect("[", "after let body")?;
        self.expect("/", "after let body")?;
        if !self.consume("let") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/let]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "after let body")?;
        Ok(Block::Let(LetBlock {
            var_name,
            var_type_name,
            value,
            body,
        })
        .rc())
    }

    fn parse_file_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        self.skip_whitespace();
        self.expect("(", "in file")?;
        let path = self.parse_expression()?;
        self.skip_whitespace();
        let mut append = false;
        let mut charset = String::new();
        if self.consume(",") {
            self.skip_whitespace();
            if self.peek("true") {
                self.consume("true");
                append = true;
            } else if self.peek("false") {
                self.consume("false");
                append = false;
            }
            self.skip_whitespace();
            if self.consume(",") {
                charset = self.parse_string_literal()?;
            }
        }
        self.expect(")", "in file")?;
        self.expect("]", "to end file header")?;
        self.skip_line_after_tag();
        let body = self.parse_blocks(&["file"])?;
        self.expect("[", "after file body")?;
        self.expect("/", "after file body")?;
        if !self.consume("file") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/file]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "after file body")?;
        Ok(Block::File(FileBlock {
            path,
            append,
            charset,
            body,
        })
        .rc())
    }

    fn parse_protected_block(&mut self) -> Result<Rc<Block>, AcceleoParseError> {
        self.skip_whitespace();
        let mut id = String::new();
        if self.consume("(") {
            self.skip_whitespace();
            if self.pos < self.bytes.len()
                && (self.bytes[self.pos] == b'\'' || self.bytes[self.pos] == b'"')
            {
                id = self.parse_string_literal()?;
            } else {
                id = self.parse_identifier()?;
            }
            self.expect(")", "in protected")?;
        }
        self.expect("]", "to end protected header")?;
        self.skip_line_after_tag();
        let body = self.parse_blocks(&["protected"])?;
        self.expect("[", "after protected body")?;
        self.expect("/", "after protected body")?;
        if !self.consume("protected") {
            return Err(AcceleoParseError::new(format!(
                "expected '[/protected]' at pos {}",
                self.pos
            )));
        }
        self.expect("]", "after protected body")?;
        Ok(Block::Protected(ProtectedBlock { id, body }).rc())
    }

    // ===== expressions =====

    fn parse_expression(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        let mut left = self.parse_and()?;
        self.skip_whitespace();
        while self.peek("or") {
            self.consume("or");
            let right = self.parse_and()?;
            left = Expr::Call {
                target: None,
                name: "or".into(),
                args: vec![left, right],
                arrow: false,
            }
            .rc();
            self.skip_whitespace();
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        let mut left = self.parse_equality()?;
        self.skip_whitespace();
        while self.peek("and") {
            self.consume("and");
            let right = self.parse_equality()?;
            left = Expr::Call {
                target: None,
                name: "and".into(),
                args: vec![left, right],
                arrow: false,
            }
            .rc();
            self.skip_whitespace();
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        let mut left = self.parse_additive()?;
        self.skip_whitespace();
        loop {
            let op = if self.peek("==") {
                self.consume("==");
                "=="
            } else if self.peek("!=") {
                self.consume("!=");
                "!="
            } else if self.peek("<>") {
                self.consume("<>");
                "!="
            } else if self.peek("=") {
                // AQL uses `=` for equality (OCL style).
                self.consume("=");
                "=="
            } else {
                break;
            };
            let right = self.parse_additive()?;
            left = Expr::Call {
                target: None,
                name: op.into(),
                args: vec![left, right],
                arrow: false,
            }
            .rc();
            self.skip_whitespace();
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        let mut left = self.parse_conditional()?;
        self.skip_whitespace();
        while self.peek("+") || self.peek("-") {
            let op = if self.peek("+") {
                self.consume("+");
                "+"
            } else {
                self.consume("-");
                "-"
            };
            let right = self.parse_conditional()?;
            left = Expr::Call {
                target: Some(left),
                name: op.into(),
                args: vec![right],
                arrow: false,
            }
            .rc();
            self.skip_whitespace();
        }
        Ok(left)
    }

    fn parse_conditional(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        let cond = self.parse_postfix()?;
        self.skip_whitespace();
        if self.peek("?") {
            self.consume("?");
            let then_expr = self.parse_expression()?;
            self.skip_whitespace();
            self.expect(":", "in conditional")?;
            let else_expr = self.parse_expression()?;
            return Ok(Expr::If {
                cond,
                then_expr,
                else_expr,
            }
            .rc());
        }
        Ok(cond)
    }

    fn parse_primary(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        self.skip_whitespace();
        // String literal.
        if self.pos < self.bytes.len()
            && (self.bytes[self.pos] == b'\'' || self.bytes[self.pos] == b'"')
        {
            return Ok(Expr::StringLit(self.parse_string_literal()?).rc());
        }
        // Integer.
        if self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_digit()
                || (self.bytes[self.pos] == b'-'
                    && self.pos + 1 < self.bytes.len()
                    && self.bytes[self.pos + 1].is_ascii_digit()))
        {
            return Ok(Expr::IntLit(self.parse_integer()?).rc());
        }
        if self.peek("true") {
            self.consume("true");
            return Ok(Expr::BoolLit(true).rc());
        }
        if self.peek("false") {
            self.consume("false");
            return Ok(Expr::BoolLit(false).rc());
        }
        if self.peek("(") {
            self.consume("(");
            let inner = self.parse_expression()?;
            self.expect(")", "in parenthesized expr")?;
            return Ok(inner);
        }
        // Bare identifier: a variable, or a global call `name(args)`.
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        if self.peek("(") {
            self.consume("(");
            let mut args = Vec::new();
            if !self.peek(")") {
                args.push(self.parse_expression()?);
                while self.consume(",") {
                    args.push(self.parse_expression()?);
                }
            }
            self.expect(")", "in bare call args")?;
            return Ok(Expr::Call {
                target: None,
                name,
                args,
                arrow: false,
            }
            .rc());
        }
        Ok(Expr::Var(name).rc())
    }

    fn parse_postfix(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        let mut e = self.parse_primary()?;
        loop {
            self.skip_whitespace();
            if self.peek("->") {
                self.consume("->");
                let name = self.parse_identifier()?;
                e = self.parse_call_args(e, name, true)?;
            } else if self.peek(".") {
                let saved = self.pos;
                self.consume(".");
                if self.pos < self.bytes.len() && is_ident_start(self.bytes[self.pos]) {
                    let name = self.parse_identifier()?;
                    e = self.parse_call_args(e, name, false)?;
                } else {
                    self.pos = saved;
                    break;
                }
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_call_args(
        &mut self,
        target: Rc<Expr>,
        name: String,
        arrow: bool,
    ) -> Result<Rc<Expr>, AcceleoParseError> {
        self.skip_whitespace();
        if self.peek("(") {
            self.consume("(");
            let mut args = Vec::new();
            if !self.peek(")") {
                args.push(self.parse_call_arg()?);
                while self.consume(",") {
                    args.push(self.parse_call_arg()?);
                }
            }
            self.expect(")", "in call args")?;
            return Ok(Expr::Call {
                target: Some(target),
                name,
                args,
                arrow,
            }
            .rc());
        }
        if arrow {
            // `->name` without `()` is a zero-arg call (e.g. `->size`).
            Ok(Expr::Call {
                target: Some(target),
                name,
                args: Vec::new(),
                arrow: true,
            }
            .rc())
        } else {
            Ok(Expr::Nav { target, name }.rc())
        }
    }

    /// Parse a single call argument: a lambda `var | body` if the shape fits,
    /// otherwise a plain expression.
    fn parse_call_arg(&mut self) -> Result<Rc<Expr>, AcceleoParseError> {
        self.skip_whitespace();
        let saved = self.pos;
        if self.pos < self.bytes.len()
            && (is_ident_start(self.bytes[self.pos]) || self.bytes[self.pos] == b'_')
        {
            let id_start = self.pos;
            while self.pos < self.bytes.len() && is_ident_part(self.bytes[self.pos]) {
                self.pos += 1;
            }
            let id = self.src[id_start..self.pos].to_string();
            self.skip_whitespace();
            if self.peek("|") {
                self.consume("|");
                let body = self.parse_expression()?;
                return Ok(Expr::Lambda { var_name: id, body }.rc());
            }
            self.pos = saved;
        }
        self.parse_expression()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_module_and_template() {
        let src = r#"
[module gen(c : Class)]
[template public genClass(c : Class)]
class [c.name/] {
}
[/template]
[/module]
"#;
        let m = parse(src).expect("parse ok");
        assert_eq!(m.name, "gen");
        assert_eq!(m.params.len(), 1);
        assert_eq!(m.params[0].name, "c");
        assert_eq!(m.templates.len(), 1);
        assert_eq!(m.templates[0].name, "genClass");
        assert_eq!(m.templates[0].params.len(), 1);
    }

    #[test]
    fn parses_expr_block_sequence() {
        let src = "[module t(c : Class)]\n[template public f(c : Class)]Hello [c.name/]![/template]\n[/module]";
        let m = parse(src).expect("parse ok");
        assert_eq!(m.templates[0].body.len(), 3);
    }

    #[test]
    fn parses_query() {
        let src = "[module t(c : Class)]\n[query public double(x : String) : String = 'X' + x /]\n[template public f(c : Class)][/template]\n[/module]";
        let m = parse(src).expect("parse ok");
        assert_eq!(m.queries.len(), 1);
        assert_eq!(m.queries[0].name, "double");
        assert_eq!(m.queries[0].return_type_name, "String");
    }

    #[test]
    fn parses_for_with_nested_if() {
        let src = r#"
[module t(c : Class)]
[template public f(c : Class)]
[for (a | c.attributes)]
[if (a.name = 'id')]ID[a.name/][else][a.name/][/if]
[/for]
[/template]
[/module]
"#;
        let m = parse(src).expect("parse ok");
        let has_for = m.templates[0]
            .body
            .iter()
            .any(|b| matches!(&**b, Block::For(_)));
        assert!(has_for);
    }
}
