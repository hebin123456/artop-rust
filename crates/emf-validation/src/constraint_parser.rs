//! OCL-subset constraint expression parser (recursive descent), a faithful Rust
//! port of C++ `emf-validation/src/ConstraintParser.cpp` aligned to the
//! Eclipse OCL / EMF Validation eval semantics.
//!
//! Supported grammar (precedence low -> high):
//!   implies(or/xor/and/not/!), comparisons (= == <> != > < >= <=), arithmetic
//!   (+ - * /), collection iterators (forAll/exists/collect/select/reject/any/
//!   iterate), collection ops (size/isEmpty/notEmpty/sortedBy/first/last/at/
//!   indexOf/count/includes/excludes/includesAll/excludesAll/union/intersection/
//!   difference/flatten/sum/asSet/asSequence/asBag/asOrderedSet), path
//!   navigation (self/iter-var/let-var/implicit-self/deep paths), literals
//!   (null/'str'/str/true/false/numbers incl. negation), if-then-else endif,
//!   let bindings, Tuple literals + field access, String/Integer method
//!   libraries, object ops (oclIsUndefined/oclIsInvalid always false/
//!   oclIsKindOf/oclIsTypeOf/asSequence), and the `value` constraint argument.
//!
//! OCL semantics notes:
//!   - empty collection forAll -> true, exists -> false;
//!   - implies: A implies B = (not A) or B (right associative);
//!   - `/<>` are OCL standard equality (also accept ==/!=);
//!   - iterating a single-valued reference treats it as a one-element set;
//!   - parse failure / empty expression returns a constant-true evaluator.
//!
//! Rust adaptation: `Evaluator` is `dyn Fn(&dyn EObject) -> bool` and carries no
//! external `value`. `compile` wraps a value-capable evaluator value-less; the
//! richer `compile_value` accepts an external `Option<Val>` for constraints that
//! reference `value`. Because Rust has no null target, the C++ "null target
//! passes" nicety is naturally satisfied (the target is always live).

use crate::constraint::{Constraint, ConstraintMode, Evaluator, Severity};
use crate::e_validator::EValidator;
use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use std::rc::Rc;

// ===================== runtime value =====================
// Analogue of C++ `std::any`, distinguishing navigation-produced EList
// collections (`Nav`, NOT auto-flattened by `collect`) from computed result
// collections (`Seq`, auto-flattened by `collect`) to reproduce the C++ port.
#[derive(Clone)]
enum V {
    /// `self` / the current evaluation target (never materialized as ObjectRef).
    This,
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
    Object(ObjectRef),
    /// A computed collection (collect/select/... results). Flattened by collect.
    Seq(Vec<V>),
    /// A navigation-produced EList (many-valued feature read). NOT flattened.
    Nav(Vec<V>),
    /// OCL Tuple literal: ordered named parts.
    Tuple(Vec<(String, V)>),
}

// ===================== value helpers =====================

fn val_to_v(val: &Val) -> V {
    match val {
        Val::Int(i) => V::Int(*i),
        Val::Double(d) => V::Double(*d),
        Val::String(s) => V::String(s.clone()),
        Val::Bool(b) => V::Bool(*b),
        Val::Byte(b) => V::Int(*b as i64),
        Val::EnumLiteral(e) => V::String(e.clone()),
        Val::Object(o) => V::Object(o.clone()),
        Val::List(l) => V::Nav(l.iter().map(val_to_v).collect()),
        Val::Null => V::Null,
    }
}

fn read_attr(target: &dyn EObject, attr_name: &str) -> V {
    match target.e_get(attr_name) {
        Some(val) => val_to_v(&val),
        None => V::Null,
    }
}

fn v_as_number(v: &V) -> Option<f64> {
    match v {
        V::Int(i) => Some(*i as f64),
        V::Double(d) => Some(*d),
        _ => None,
    }
}

fn v_as_string(v: &V) -> Option<&str> {
    match v {
        V::String(s) => Some(s),
        _ => None,
    }
}

fn v_as_object(v: &V) -> Option<&ObjectRef> {
    match v {
        V::Object(o) => Some(o),
        _ => None,
    }
}

fn v_is_null(v: &V) -> bool {
    matches!(v, V::Null)
}

fn v_is_collection(v: &V) -> bool {
    matches!(v, V::Seq(_) | V::Nav(_))
}

fn to_bool(v: &V) -> bool {
    match v {
        V::This | V::Null => false,
        V::Bool(b) => *b,
        V::Int(i) => *i != 0,
        V::Double(d) => *d != 0.0,
        V::String(s) => !s.is_empty(),
        V::Object(_) => true,
        V::Seq(_) | V::Nav(_) => false,
        V::Tuple(_) => false,
    }
}

fn is_null_value(v: &V) -> bool {
    v_is_null(v)
}

/// Collection multiset equality (order-independent, duplicate-counting).
fn collections_equal(a: &[V], b: &[V]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut used: Vec<bool> = vec![false; b.len()];
    for av in a {
        let mut found = false;
        for (i, bv) in b.iter().enumerate() {
            if !used[i] && values_equal(av, bv) {
                used[i] = true;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

fn values_equal(a: &V, b: &V) -> bool {
    let a_null = is_null_value(a);
    let b_null = is_null_value(b);
    if a_null || b_null {
        return a_null && b_null;
    }
    if v_is_collection(a) || v_is_collection(b) {
        if !v_is_collection(a) || !v_is_collection(b) {
            return false;
        }
        return collections_equal(&as_any_list(a), &as_any_list(b));
    }
    if let (Some(as_), Some(bs)) = (v_as_string(a), v_as_string(b)) {
        return as_ == bs;
    }
    if let (Some(an), Some(bn)) = (v_as_number(a), v_as_number(b)) {
        return an == bn;
    }
    if let (V::Bool(ab), V::Bool(bb)) = (a, b) {
        return ab == bb;
    }
    if let (Some(oa), Some(ob)) = (v_as_object(a), v_as_object(b)) {
        return Rc::ptr_eq(oa, ob);
    }
    if let (V::Tuple(ta), V::Tuple(tb)) = (a, b) {
        if ta.len() != tb.len() {
            return false;
        }
        for (an, av) in ta {
            match tb.iter().find(|(bn, _)| bn == an) {
                Some((_, bv)) => {
                    if !values_equal(av, bv) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        return true;
    }
    false
}

/// General element list (value may be collection / single object / scalar).
fn as_any_list(v: &V) -> Vec<V> {
    match v {
        V::Seq(l) | V::Nav(l) => l.clone(),
        V::Object(o) => vec![V::Object(o.clone())],
        V::Null | V::This => Vec::new(),
        other => vec![other.clone()],
    }
}

/// EObject element list (used for iteration over object collections).
fn as_element_list(v: &V) -> Vec<ObjectRef> {
    match v {
        V::Object(o) => vec![o.clone()],
        V::Seq(l) | V::Nav(l) => l.iter().filter_map(|x| v_as_object(x).cloned()).collect(),
        _ => Vec::new(),
    }
}

fn size_of(v: &V) -> i64 {
    match v {
        V::Seq(l) | V::Nav(l) => l.len() as i64,
        V::String(s) => s.chars().count() as i64,
        _ => 0,
    }
}

// ===================== evaluation context =====================

/// Eval context: `value` constraint arg, iterator-var bindings and let-var
/// bindings (scoped stacks supporting nesting). The evaluation target (`self`)
/// is threaded separately through each eval call.
struct EvalCtx {
    value: Option<Val>,
    vars: Vec<(String, ObjectRef)>,
    lets: Vec<(String, V)>,
}

impl EvalCtx {
    fn new(value: Option<Val>) -> Self {
        Self {
            value,
            vars: Vec::new(),
            lets: Vec::new(),
        }
    }
    fn is_bound_var(&self, n: &str) -> bool {
        self.vars.iter().rev().any(|(name, _)| name == n)
    }
    fn lookup_var(&self, n: &str) -> Option<ObjectRef> {
        self.vars
            .iter()
            .rev()
            .find(|(name, _)| name == n)
            .map(|(_, o)| o.clone())
    }
    fn lookup_let_var(&self, n: &str) -> Option<V> {
        self.lets
            .iter()
            .rev()
            .find(|(name, _)| name == n)
            .map(|(_, v)| v.clone())
    }
}

/// Unified expression evaluator. Each eval receives the mutable context plus
/// the evaluation target (`self`), so a recursive AST closure needs no
/// higher-ranked lifetime gymnastics.
type ExprEval = Box<dyn Fn(&mut EvalCtx, &dyn EObject) -> V>;

// ===================== lexer =====================

#[derive(Clone, PartialEq)]
enum Tok {
    Ident(String),
    Number(f64),
    String(String),
    Arrow,
    Dot,
    Pipe,
    Comma,
    Minus,
    Plus,
    Star,
    Slash,
    Colon,
    Semicolon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    RelOp(String),
    And,
    Or,
    Not,
    Xor,
    Implies,
    True,
    False,
    Null,
    If,
    Then,
    Else,
    Endif,
    Let,
    In,
    Tuple,
    End,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn tokenize(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let n = chars.len();
    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // two-char operators / arrows
        if c == '-' && i + 1 < n && chars[i + 1] == '>' {
            out.push(Tok::Arrow);
            i += 2;
            continue;
        }
        if c == '-' {
            out.push(Tok::Minus);
            i += 1;
            continue;
        }
        if c == '+' {
            out.push(Tok::Plus);
            i += 1;
            continue;
        }
        if c == '*' {
            out.push(Tok::Star);
            i += 1;
            continue;
        }
        if c == '/' {
            out.push(Tok::Slash);
            i += 1;
            continue;
        }
        if c == ':' {
            out.push(Tok::Colon);
            i += 1;
            continue;
        }
        if c == ';' {
            out.push(Tok::Semicolon);
            i += 1;
            continue;
        }
        if c == '.' {
            out.push(Tok::Dot);
            i += 1;
            continue;
        }
        if c == '|' {
            if i + 1 < n && chars[i + 1] == '|' {
                out.push(Tok::Or);
                i += 2;
            } else {
                out.push(Tok::Pipe);
                i += 1;
            }
            continue;
        }
        if c == ',' {
            out.push(Tok::Comma);
            i += 1;
            continue;
        }
        if c == '(' {
            out.push(Tok::LParen);
            i += 1;
            continue;
        }
        if c == ')' {
            out.push(Tok::RParen);
            i += 1;
            continue;
        }
        if c == '{' {
            out.push(Tok::LBrace);
            i += 1;
            continue;
        }
        if c == '}' {
            out.push(Tok::RBrace);
            i += 1;
            continue;
        }
        if c == '&' && i + 1 < n && chars[i + 1] == '&' {
            out.push(Tok::And);
            i += 2;
            continue;
        }
        if c == '!' {
            if i + 1 < n && chars[i + 1] == '=' {
                out.push(Tok::RelOp("!=".to_string()));
                i += 2;
            } else {
                out.push(Tok::Not);
                i += 1;
            }
            continue;
        }
        if c == '=' {
            if i + 1 < n && chars[i + 1] == '=' {
                out.push(Tok::RelOp("==".to_string()));
                i += 2;
            } else {
                out.push(Tok::RelOp("=".to_string()));
                i += 1;
            }
            continue;
        }
        if c == '<' {
            if i + 1 < n && chars[i + 1] == '=' {
                out.push(Tok::RelOp("<=".to_string()));
                i += 2;
            } else if i + 1 < n && chars[i + 1] == '>' {
                out.push(Tok::RelOp("<>".to_string()));
                i += 2;
            } else {
                out.push(Tok::RelOp("<".to_string()));
                i += 1;
            }
            continue;
        }
        if c == '>' {
            if i + 1 < n && chars[i + 1] == '=' {
                out.push(Tok::RelOp(">=".to_string()));
                i += 2;
            } else {
                out.push(Tok::RelOp(">".to_string()));
                i += 1;
            }
            continue;
        }
        // string literal (' or ")
        if c == '\'' || c == '"' {
            let q = c;
            i += 1;
            let mut str = String::new();
            while i < n && chars[i] != q {
                str.push(chars[i]);
                i += 1;
            }
            if i < n {
                i += 1; // skip closing quote
            }
            out.push(Tok::String(str));
            continue;
        }
        // number literal
        if c.is_ascii_digit() {
            let mut num = String::new();
            while i < n && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num.push(chars[i]);
                i += 1;
            }
            let val: f64 = num.parse().unwrap_or(0.0);
            out.push(Tok::Number(val));
            continue;
        }
        // identifier / keyword
        if is_ident_start(c) {
            let mut id = String::new();
            while i < n && is_ident_char(chars[i]) {
                id.push(chars[i]);
                i += 1;
            }
            let t = match id.as_str() {
                "and" => Tok::And,
                "or" => Tok::Or,
                "not" => Tok::Not,
                "xor" => Tok::Xor,
                "implies" => Tok::Implies,
                "true" => Tok::True,
                "false" => Tok::False,
                "null" => Tok::Null,
                "if" => Tok::If,
                "then" => Tok::Then,
                "else" => Tok::Else,
                "endif" => Tok::Endif,
                "let" => Tok::Let,
                "in" => Tok::In,
                "Tuple" => Tok::Tuple,
                _ => Tok::Ident(id),
            };
            out.push(t);
            continue;
        }
        // unknown single char: skip (parse will fail on leftover tokens)
        i += 1;
    }
    out.push(Tok::End);
    out
}

// ===================== recursive descent parser =====================

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    ok: bool,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Self {
            toks,
            pos: 0,
            ok: true,
        }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn advance(&mut self) {
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, t: Tok) {
        if self.peek() == &t {
            self.advance();
        } else {
            self.ok = false;
        }
    }

    fn expect_assign(&mut self) {
        if let Tok::RelOp(op) = self.peek() {
            if op == "=" || op == "==" {
                self.advance();
                return;
            }
        }
        self.ok = false;
    }

    fn parse_optional_type(&mut self) {
        if self.peek() == &Tok::Colon {
            self.advance();
            if let Tok::Ident(_) = self.peek() {
                self.advance();
            } else {
                self.ok = false;
            }
        }
    }

    fn parse_top(&mut self) -> ExprEval {
        let e = self.parse_implies();
        if self.peek() != &Tok::End {
            self.ok = false;
        }
        e
    }

    // implies (right-assoc, lowest) : A implies B = (not A) or B
    fn parse_implies(&mut self) -> ExprEval {
        let left = self.parse_or();
        if self.peek() == &Tok::Implies {
            self.advance();
            let right = self.parse_implies();
            return Box::new(move |ctx, target| {
                V::Bool(!to_bool(&left(ctx, target)) || to_bool(&right(ctx, target)))
            });
        }
        left
    }

    fn parse_or(&mut self) -> ExprEval {
        let mut left = self.parse_xor();
        while self.peek() == &Tok::Or {
            self.advance();
            let right = self.parse_xor();
            left = Box::new(move |ctx, target| {
                V::Bool(to_bool(&left(ctx, target)) || to_bool(&right(ctx, target)))
            });
        }
        left
    }

    fn parse_xor(&mut self) -> ExprEval {
        let mut left = self.parse_and();
        while self.peek() == &Tok::Xor {
            self.advance();
            let right = self.parse_and();
            left = Box::new(move |ctx, target| {
                V::Bool(to_bool(&left(ctx, target)) != to_bool(&right(ctx, target)))
            });
        }
        left
    }

    fn parse_and(&mut self) -> ExprEval {
        let mut left = self.parse_unary();
        while self.peek() == &Tok::And {
            self.advance();
            let right = self.parse_unary();
            left = Box::new(move |ctx, target| {
                V::Bool(to_bool(&left(ctx, target)) && to_bool(&right(ctx, target)))
            });
        }
        left
    }

    fn parse_unary(&mut self) -> ExprEval {
        if self.peek() == &Tok::Not {
            self.advance();
            let operand = self.parse_unary();
            return Box::new(move |ctx, target| {
                V::Bool(!to_bool(&operand(ctx, target)))
            });
        }
        if self.peek() == &Tok::Minus {
            self.advance();
            let operand = self.parse_unary();
            return Box::new(move |ctx, target| {
                match v_as_number(&operand(ctx, target)) {
                    Some(n) => V::Double(-n),
                    None => V::Null,
                }
            });
        }
        self.parse_relational()
    }

    fn parse_relational(&mut self) -> ExprEval {
        let left = self.parse_additive();
        if let Tok::RelOp(op) = self.peek() {
            let op = op.clone();
            self.advance();
            let right = self.parse_additive();
            return self.make_comparison(left, &op, right);
        }
        left
    }

    fn parse_additive(&mut self) -> ExprEval {
        let mut left = self.parse_multiplicative();
        loop {
            match self.peek() {
                Tok::Plus => {
                    self.advance();
                    let right = self.parse_multiplicative();
                    left = Box::new(move |ctx, target| {
                        let lv = left(ctx, target);
                        let rv = right(ctx, target);
                        if let (Some(ls), Some(rs)) = (v_as_string(&lv), v_as_string(&rv)) {
                            return V::String(format!("{ls}{rs}"));
                        }
                        if let (Some(ln), Some(rn)) = (v_as_number(&lv), v_as_number(&rv)) {
                            return V::Double(ln + rn);
                        }
                        V::Null
                    });
                }
                Tok::Minus => {
                    self.advance();
                    let right = self.parse_multiplicative();
                    left = Box::new(move |ctx, target| {
                        if let (Some(ln), Some(rn)) = (
                            v_as_number(&left(ctx, target)),
                            v_as_number(&right(ctx, target)),
                        ) {
                            return V::Double(ln - rn);
                        }
                        V::Null
                    });
                }
                _ => break,
            }
        }
        left
    }

    fn parse_multiplicative(&mut self) -> ExprEval {
        let mut left = self.parse_primary();
        loop {
            match self.peek() {
                Tok::Star => {
                    self.advance();
                    let right = self.parse_primary();
                    left = Box::new(move |ctx, target| {
                        if let (Some(ln), Some(rn)) = (
                            v_as_number(&left(ctx, target)),
                            v_as_number(&right(ctx, target)),
                        ) {
                            return V::Double(ln * rn);
                        }
                        V::Null
                    });
                }
                Tok::Slash => {
                    self.advance();
                    let right = self.parse_primary();
                    left = Box::new(move |ctx, target| {
                        if let (Some(ln), Some(rn)) = (
                            v_as_number(&left(ctx, target)),
                            v_as_number(&right(ctx, target)),
                        ) {
                            if rn != 0.0 {
                                return V::Double(ln / rn);
                            }
                        }
                        V::Null
                    });
                }
                _ => break,
            }
        }
        left
    }

    fn make_comparison(&self, left: ExprEval, op: &str, right: ExprEval) -> ExprEval {
        let is_eq = op == "=" || op == "==";
        let is_neq = op == "<>" || op == "!=";
        if is_eq || is_neq {
            let is_neq = is_neq;
            return Box::new(move |ctx, target| {
                let eq = values_equal(&left(ctx, target), &right(ctx, target));
                V::Bool(if is_neq { !eq } else { eq })
            });
        }
        let op = op.to_string();
        Box::new(move |ctx, target| {
            let lv = left(ctx, target);
            let rv = right(ctx, target);
            if let (Some(ln), Some(rn)) = (v_as_number(&lv), v_as_number(&rv)) {
                let b = match op.as_str() {
                    ">" => ln > rn,
                    "<" => ln < rn,
                    ">=" => ln >= rn,
                    "<=" => ln <= rn,
                    _ => true,
                };
                return V::Bool(b);
            }
            if let (Some(ls), Some(rs)) = (v_as_string(&lv), v_as_string(&rv)) {
                let b = match op.as_str() {
                    ">" => ls > rs,
                    "<" => ls < rs,
                    ">=" => ls >= rs,
                    "<=" => ls <= rs,
                    _ => true,
                };
                return V::Bool(b);
            }
            V::Bool(true) // non-numeric/string: tolerant pass
        })
    }

    fn parse_primary(&mut self) -> ExprEval {
        if self.peek() == &Tok::LParen {
            self.advance();
            let e = self.parse_implies();
            self.expect(Tok::RParen);
            return e;
        }
        if self.peek() == &Tok::If {
            return self.parse_if();
        }
        if self.peek() == &Tok::Let {
            return self.parse_let();
        }
        if self.peek() == &Tok::Tuple {
            return self.parse_tuple_literal();
        }
        let mut src = self.parse_atom();
        while self.ok && matches!(self.peek(), Tok::Dot | Tok::Arrow) {
            if self.peek() == &Tok::Dot {
                self.advance();
                src = self.parse_dot_postfix(src);
            } else {
                self.advance();
                src = self.parse_arrow_postfix(src);
            }
        }
        src
    }

    fn parse_if(&mut self) -> ExprEval {
        self.expect(Tok::If);
        let cond = self.parse_implies();
        self.expect(Tok::Then);
        let then_e = self.parse_implies();
        self.expect(Tok::Else);
        let else_e = self.parse_implies();
        self.expect(Tok::Endif);
        Box::new(move |ctx, target| {
            if to_bool(&cond(ctx, target)) {
                then_e(ctx, target)
            } else {
                else_e(ctx, target)
            }
        })
    }

    fn parse_let(&mut self) -> ExprEval {
        self.expect(Tok::Let);
        let var_name = if let Tok::Ident(n) = self.peek() {
            let n = n.clone();
            self.advance();
            n
        } else {
            self.ok = false;
            String::new()
        };
        self.parse_optional_type();
        self.expect_assign();
        let init_expr = self.parse_implies();
        self.expect(Tok::In);
        let body = self.parse_implies();
        Box::new(move |ctx, target| {
            let init_val = init_expr(ctx, target);
            ctx.lets.push((var_name.clone(), init_val));
            let result = body(ctx, target);
            ctx.lets.pop();
            result
        })
    }

    fn parse_tuple_literal(&mut self) -> ExprEval {
        self.expect(Tok::Tuple);
        self.expect(Tok::LBrace);
        let mut part_evals: Vec<(String, ExprEval)> = Vec::new();
        if self.peek() != &Tok::RBrace {
            loop {
                if let Tok::Ident(name) = self.peek() {
                    let name = name.clone();
                    self.advance();
                    self.parse_optional_type();
                    self.expect_assign();
                    let val_expr = self.parse_implies();
                    part_evals.push((name, val_expr));
                } else {
                    self.ok = false;
                    break;
                }
                if self.peek() == &Tok::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(Tok::RBrace);
        Box::new(move |ctx, target| {
            let mut parts: Vec<(String, V)> = Vec::new();
            for (name, expr) in &part_evals {
                parts.push((name.clone(), expr(ctx, target)));
            }
            V::Tuple(parts)
        })
    }

    fn parse_atom(&mut self) -> ExprEval {
        match self.peek().clone() {
            Tok::Null => {
                self.advance();
                Box::new(|_, _| V::Null)
            }
            Tok::True => {
                self.advance();
                Box::new(|_, _| V::Bool(true))
            }
            Tok::False => {
                self.advance();
                Box::new(|_, _| V::Bool(false))
            }
            Tok::Number(v) => {
                self.advance();
                Box::new(move |_, _| V::Double(v))
            }
            Tok::String(v) => {
                self.advance();
                Box::new(move |_, _| V::String(v.clone()))
            }
            Tok::Ident(first) => {
                self.advance();
                Box::new(move |ctx, target| {
                    if first == "value" {
                        return match &ctx.value {
                            Some(v) => val_to_v(v),
                            None => V::Null,
                        };
                    }
                    if first == "self" {
                        return V::This;
                    }
                    if let Some(v) = ctx.lookup_let_var(&first) {
                        return v;
                    }
                    if ctx.is_bound_var(&first) {
                        if let Some(o) = ctx.lookup_var(&first) {
                            return V::Object(o);
                        }
                    }
                    read_attr(target, &first) // implicit self.attr
                })
            }
            _ => {
                self.ok = false;
                if self.peek() != &Tok::End {
                    self.advance();
                }
                Box::new(|_, _| V::Null)
            }
        }
    }

    fn parse_dot_postfix(&mut self, src: ExprEval) -> ExprEval {
        let name = if let Tok::Ident(n) = self.peek() {
            let n = n.clone();
            self.advance();
            n
        } else {
            self.ok = false;
            return src;
        };

        if self.peek() == &Tok::LParen {
            // method call
            self.advance();
            let mut args: Vec<ExprEval> = Vec::new();
            if self.peek() != &Tok::RParen {
                if name == "oclIsKindOf" || name == "oclIsTypeOf" {
                    if let Tok::Ident(type_name) = self.peek() {
                        let type_name = type_name.clone();
                        self.advance();
                        args.push(Box::new(move |_, _| V::String(type_name.clone())));
                    } else {
                        self.ok = false;
                    }
                } else {
                    args.push(self.parse_implies());
                    while self.peek() == &Tok::Comma {
                        self.advance();
                        args.push(self.parse_implies());
                    }
                }
            }
            self.expect(Tok::RParen);
            return self.make_method_call(src, &name, args);
        }
        // property navigation
        Box::new(move |ctx, target| {
            let val = src(ctx, target);
            match val {
                V::This => read_attr(target, &name),
                V::Tuple(parts) => {
                    match parts.iter().find(|(n, _)| n == &name) {
                        Some((_, v)) => v.clone(),
                        None => V::Null,
                    }
                }
                V::Object(o) => {
                    let oo = o.borrow();
                    read_attr(&*oo, &name)
                }
                _ => V::Null,
            }
        })
    }

    fn parse_arrow_postfix(&mut self, src: ExprEval) -> ExprEval {
        let op_name = if let Tok::Ident(n) = self.peek() {
            let n = n.clone();
            self.advance();
            n
        } else {
            self.ok = false;
            return src;
        };

        if op_name == "forAll" || op_name == "exists" {
            self.expect(Tok::LParen);
            let var_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.parse_optional_type();
            self.expect(Tok::Pipe);
            let body = self.parse_implies();
            self.expect(Tok::RParen);
            return Self::make_iterator(src, &op_name, var_name, body);
        }
        if op_name == "collect" {
            self.expect(Tok::LParen);
            let var_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.parse_optional_type();
            self.expect(Tok::Pipe);
            let body = self.parse_implies();
            self.expect(Tok::RParen);
            return Self::make_collect(src, var_name, body);
        }
        if op_name == "select" || op_name == "reject" {
            self.expect(Tok::LParen);
            let var_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.parse_optional_type();
            self.expect(Tok::Pipe);
            let body = self.parse_implies();
            self.expect(Tok::RParen);
            return Self::make_select_reject(src, &op_name, var_name, body);
        }
        if op_name == "any" {
            self.expect(Tok::LParen);
            let var_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.parse_optional_type();
            self.expect(Tok::Pipe);
            let body = self.parse_implies();
            self.expect(Tok::RParen);
            return Self::make_any(src, var_name, body);
        }
        if op_name == "iterate" {
            self.expect(Tok::LParen);
            let var_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.expect(Tok::Semicolon);
            let acc_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.parse_optional_type();
            self.expect_assign();
            let init = self.parse_implies();
            self.expect(Tok::Pipe);
            let body = self.parse_implies();
            self.expect(Tok::RParen);
            return Self::make_iterate(src, var_name, acc_name, init, body);
        }
        if op_name == "size" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| V::Int(size_of(&src(ctx, target))));
        }
        if op_name == "isEmpty" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| V::Bool(size_of(&src(ctx, target)) == 0));
        }
        if op_name == "notEmpty" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| V::Bool(size_of(&src(ctx, target)) > 0));
        }
        if op_name == "sortedBy" {
            self.expect(Tok::LParen);
            let var_name = if let Tok::Ident(n) = self.peek() {
                let n = n.clone();
                self.advance();
                n
            } else {
                self.ok = false;
                String::new()
            };
            self.parse_optional_type();
            self.expect(Tok::Pipe);
            let body = self.parse_implies();
            self.expect(Tok::RParen);
            return Self::make_sorted_by(src, var_name, body);
        }
        if op_name == "first" || op_name == "last" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            let is_first = op_name == "first";
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                if list.is_empty() {
                    return V::Null;
                }
                if is_first {
                    list.first().cloned().unwrap_or(V::Null)
                } else {
                    list.last().cloned().unwrap_or(V::Null)
                }
            });
        }
        if op_name == "at" {
            self.expect(Tok::LParen);
            let arg = self.parse_implies();
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                match v_as_number(&arg(ctx, target)) {
                    Some(idx) => {
                        let i = idx as i64;
                        if i < 1 || i > list.len() as i64 {
                            V::Null
                        } else {
                            list[(i - 1) as usize].clone()
                        }
                    }
                    None => V::Null,
                }
            });
        }
        if op_name == "indexOf" {
            self.expect(Tok::LParen);
            let arg = self.parse_implies();
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let target_v = arg(ctx, target);
                for (i, v) in list.iter().enumerate() {
                    if values_equal(v, &target_v) {
                        return V::Int((i + 1) as i64);
                    }
                }
                V::Int(0)
            });
        }
        if op_name == "count" {
            self.expect(Tok::LParen);
            let arg = self.parse_implies();
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let target_v = arg(ctx, target);
                let mut cnt = 0;
                for v in &list {
                    if values_equal(v, &target_v) {
                        cnt += 1;
                    }
                }
                V::Int(cnt)
            });
        }
        if op_name == "includes" || op_name == "excludes" {
            self.expect(Tok::LParen);
            let arg = self.parse_implies();
            self.expect(Tok::RParen);
            let is_includes = op_name == "includes";
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let target_v = arg(ctx, target);
                for v in &list {
                    if values_equal(v, &target_v) {
                        return V::Bool(is_includes);
                    }
                }
                V::Bool(!is_includes)
            });
        }
        if op_name == "includesAll" || op_name == "excludesAll" {
            self.expect(Tok::LParen);
            let arg = self.parse_implies();
            self.expect(Tok::RParen);
            let is_includes_all = op_name == "includesAll";
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let other = as_any_list(&arg(ctx, target));
                for o in &other {
                    let found = list.iter().any(|v| values_equal(v, o));
                    if is_includes_all && !found {
                        return V::Bool(false);
                    }
                    if !is_includes_all && found {
                        return V::Bool(false);
                    }
                }
                V::Bool(true)
            });
        }
        if op_name == "union" || op_name == "intersection" || op_name == "difference" {
            self.expect(Tok::LParen);
            let arg = self.parse_implies();
            self.expect(Tok::RParen);
            let op = op_name.clone();
            return Box::new(move |ctx, target| {
                let mut a = as_any_list(&src(ctx, target));
                let b = as_any_list(&arg(ctx, target));
                if op == "union" {
                    for v in b {
                        a.push(v);
                    }
                    return V::Seq(a);
                }
                let mut consumed: Vec<bool> = vec![false; b.len()];
                let mut result: Vec<V> = Vec::new();
                for av in a {
                    let mut mi = b.len();
                    for (i, bv) in b.iter().enumerate() {
                        if !consumed[i] && values_equal(&av, bv) {
                            mi = i;
                            break;
                        }
                    }
                    if op == "intersection" {
                        if mi < b.len() {
                            consumed[mi] = true;
                            result.push(av);
                        }
                    } else {
                        // difference
                        if mi < b.len() {
                            consumed[mi] = true;
                        } else {
                            result.push(av);
                        }
                    }
                }
                V::Seq(result)
            });
        }
        if op_name == "asBag" || op_name == "asSequence" || op_name == "asOrderedSet" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| V::Seq(as_any_list(&src(ctx, target))));
        }
        if op_name == "asSet" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let mut result: Vec<V> = Vec::new();
                for v in &list {
                    if !result.iter().any(|r| values_equal(r, v)) {
                        result.push(v.clone());
                    }
                }
                V::Seq(result)
            });
        }
        if op_name == "flatten" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let mut result: Vec<V> = Vec::new();
                fn rec(v: &V, out: &mut Vec<V>) {
                    match v {
                        V::Seq(inner) | V::Nav(inner) => {
                            for e in inner {
                                rec(e, out);
                            }
                        }
                        V::This | V::Null => {}
                        other => out.push(other.clone()),
                    }
                }
                for v in &list {
                    rec(v, &mut result);
                }
                V::Seq(result)
            });
        }
        if op_name == "sum" {
            self.expect(Tok::LParen);
            self.expect(Tok::RParen);
            return Box::new(move |ctx, target| {
                let list = as_any_list(&src(ctx, target));
                let mut total = 0.0;
                let mut all_num = true;
                for v in &list {
                    match v_as_number(v) {
                        Some(n) => total += n,
                        None => {
                            all_num = false;
                            break;
                        }
                    }
                }
                if all_num {
                    return V::Double(total);
                }
                let mut acc = String::new();
                for v in &list {
                    if let Some(sv) = v_as_string(v) {
                        acc.push_str(sv);
                    }
                }
                V::String(acc)
            });
        }
        self.ok = false; // unsupported collection op
        src
    }

    fn make_iterator(source: ExprEval, op: &str, var_name: String, body: ExprEval) -> ExprEval {
        let is_forall = op == "forAll";
        Box::new(move |ctx, target| {
            let elems = as_element_list(&source(ctx, target));
            if is_forall {
                for e in &elems {
                    ctx.vars.push((var_name.clone(), e.clone()));
                    let pass = to_bool(&body(ctx, target));
                    ctx.vars.pop();
                    if !pass {
                        return V::Bool(false);
                    }
                }
                V::Bool(true)
            } else {
                for e in &elems {
                    ctx.vars.push((var_name.clone(), e.clone()));
                    let pass = to_bool(&body(ctx, target));
                    ctx.vars.pop();
                    if pass {
                        return V::Bool(true);
                    }
                }
                V::Bool(false)
            }
        })
    }

    fn make_collect(source: ExprEval, var_name: String, body: ExprEval) -> ExprEval {
        Box::new(move |ctx, target| {
            let elems = as_element_list(&source(ctx, target));
            let mut result: Vec<V> = Vec::new();
            for e in &elems {
                ctx.vars.push((var_name.clone(), e.clone()));
                let v = body(ctx, target);
                ctx.vars.pop();
                // Auto-flatten computed (Seq) results; navigation EList (Nav) stays nested.
                match v {
                    V::Seq(inner) => {
                        for x in inner {
                            result.push(x);
                        }
                    }
                    other => result.push(other),
                }
            }
            V::Seq(result)
        })
    }

    fn make_select_reject(
        source: ExprEval,
        op: &str,
        var_name: String,
        body: ExprEval,
    ) -> ExprEval {
        let is_select = op == "select";
        Box::new(move |ctx, target| {
            let elems = as_element_list(&source(ctx, target));
            let mut result: Vec<V> = Vec::new();
            for e in &elems {
                ctx.vars.push((var_name.clone(), e.clone()));
                let pass = to_bool(&body(ctx, target));
                ctx.vars.pop();
                if pass == is_select {
                    result.push(V::Object(e.clone()));
                }
            }
            V::Seq(result)
        })
    }

    fn make_any(source: ExprEval, var_name: String, body: ExprEval) -> ExprEval {
        Box::new(move |ctx, target| {
            let elems = as_element_list(&source(ctx, target));
            for e in &elems {
                ctx.vars.push((var_name.clone(), e.clone()));
                let pass = to_bool(&body(ctx, target));
                ctx.vars.pop();
                if pass {
                    return V::Object(e.clone());
                }
            }
            V::Null
        })
    }

    fn make_iterate(
        source: ExprEval,
        var_name: String,
        acc_name: String,
        init: ExprEval,
        body: ExprEval,
    ) -> ExprEval {
        Box::new(move |ctx, target| {
            let elems = as_element_list(&source(ctx, target));
            let mut acc = init(ctx, target);
            for e in &elems {
                ctx.vars.push((var_name.clone(), e.clone()));
                ctx.lets.push((acc_name.clone(), acc.clone()));
                acc = body(ctx, target);
                ctx.lets.pop();
                ctx.vars.pop();
            }
            acc
        })
    }

    fn make_sorted_by(source: ExprEval, var_name: String, body: ExprEval) -> ExprEval {
        Box::new(move |ctx, target| {
            let elems = as_element_list(&source(ctx, target));
            let mut keyed: Vec<(V, ObjectRef)> = Vec::new();
            for e in &elems {
                ctx.vars.push((var_name.clone(), e.clone()));
                let key = body(ctx, target);
                ctx.vars.pop();
                keyed.push((key, e.clone()));
            }
            keyed.sort_by(|a, b| {
                if let (Some(an), Some(bn)) = (v_as_number(&a.0), v_as_number(&b.0)) {
                    return an
                        .partial_cmp(&bn)
                        .unwrap_or(std::cmp::Ordering::Equal);
                }
                if let (Some(aa), Some(bb)) = (v_as_string(&a.0), v_as_string(&b.0)) {
                    return aa.cmp(bb);
                }
                std::cmp::Ordering::Equal
            });
            V::Seq(keyed.into_iter().map(|(_, o)| V::Object(o)).collect())
        })
    }

    fn make_method_call(
        &self,
        src: ExprEval,
        name: &str,
        args: Vec<ExprEval>,
    ) -> ExprEval {
        let name = name.to_string();
        Box::new(move |ctx, target| {
            let val = src(ctx, target);

            // generic object operations
            if name == "size" || name == "length" {
                return V::Int(size_of(&val));
            }
            if name == "oclIsUndefined" {
                return V::Bool(is_null_value(&val));
            }
            if name == "oclIsInvalid" {
                return V::Bool(false); // simplified: always false
            }
            if name == "oclType" {
                let obj: Option<&dyn EObject> = match &val {
                    V::This => ctx.target_some(target),
                    V::Object(o) => Some(&*o.borrow()),
                    _ => None,
                };
                return match obj {
                    Some(o) => V::String(o.e_class().to_string()),
                    None => V::Null,
                };
            }
            if name == "asSequence" {
                return V::Seq(as_any_list(&val));
            }
            if name == "oclIsKindOf" || name == "oclIsTypeOf" {
                let mut type_name = String::new();
                if let Some(a0) = args.first() {
                    if let Some(s) = v_as_string(&a0(ctx, target)) {
                        type_name = s.to_string();
                    }
                }
                let obj: Option<&dyn EObject> = match &val {
                    V::This => ctx.target_some(target),
                    V::Object(o) => Some(&*o.borrow()),
                    _ => None,
                };
                let Some(obj) = obj else {
                    return V::Bool(false);
                };
                let cls_name = obj.e_class().to_string();
                if name == "oclIsTypeOf" {
                    return V::Bool(cls_name == type_name);
                }
                // oclIsKindOf: self or any supertype matches
                if cls_name == type_name {
                    return V::Bool(true);
                }
                let super_names = super_type_names(obj);
                return V::Bool(super_names.iter().any(|s| s == &type_name));
            }

            // string operations
            if let Some(sv) = v_as_string(&val) {
                let s = sv; // &str
                if name == "toUpper" {
                    return V::String(s.to_uppercase());
                }
                if name == "toLower" {
                    return V::String(s.to_lowercase());
                }
                if name == "trim" {
                    return V::String(s.trim().to_string());
                }
                if name == "concat" {
                    if let Some(a0) = args.first() {
                        if let Some(other) = v_as_string(&a0(ctx, target)) {
                            return V::String(format!("{s}{other}"));
                        }
                    }
                    return V::String(s.to_string());
                }
                if name == "substring" && args.len() >= 2 {
                    let sd = v_as_number(&args[0](ctx, target));
                    let ed = v_as_number(&args[1](ctx, target));
                    if let (Some(sd), Some(ed)) = (sd, ed) {
                        let mut si = sd as i64;
                        let mut ei = ed as i64;
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i64;
                        if si < 1 {
                            si = 1;
                        }
                        if ei > len {
                            ei = len;
                        }
                        if si > ei {
                            return V::String(String::new());
                        }
                        let sub: String = chars[(si - 1) as usize..=(ei - 1) as usize]
                            .iter()
                            .collect();
                        return V::String(sub);
                    }
                    return V::String(String::new());
                }
                if name == "indexOf" && !args.is_empty() {
                    if let Some(sub) = v_as_string(&args[0](ctx, target)) {
                        return match s.find(sub) {
                            Some(pos) => V::Int((pos + 1) as i64),
                            None => V::Int(0),
                        };
                    }
                    return V::Int(0);
                }
                if name == "startsWith" && !args.is_empty() {
                    if let Some(sub) = v_as_string(&args[0](ctx, target)) {
                        return V::Bool(s.starts_with(sub));
                    }
                    return V::Bool(false);
                }
                if name == "endsWith" && !args.is_empty() {
                    if let Some(sub) = v_as_string(&args[0](ctx, target)) {
                        return V::Bool(s.ends_with(sub));
                    }
                    return V::Bool(false);
                }
                if name == "toString" {
                    return V::String(s.to_string());
                }
            }

            // integer / real operations
            if let Some(n) = v_as_number(&val) {
                if name == "abs" {
                    return V::Double(n.abs());
                }
                if name == "floor" {
                    return V::Double(n.floor());
                }
                if name == "ceil" {
                    return V::Double(n.ceil());
                }
                if name == "round" {
                    return V::Double(n.round());
                }
                if name == "toString" {
                    let s = if n.fract() == 0.0 && n.abs() < 1e15 {
                        format!("{}", n as i64)
                    } else {
                        format!("{}", n)
                    };
                    return V::String(s);
                }
                if name == "toInteger" {
                    return V::Int(n as i64);
                }
                if name == "toReal" {
                    return V::Double(n);
                }
                if name == "max" && !args.is_empty() {
                    if let Some(m) = v_as_number(&args[0](ctx, target)) {
                        return V::Double(if n > m { n } else { m });
                    }
                    return V::Double(n);
                }
                if name == "min" && !args.is_empty() {
                    if let Some(m) = v_as_number(&args[0](ctx, target)) {
                        return V::Double(if n < m { n } else { m });
                    }
                    return V::Double(n);
                }
                if name == "mod" && !args.is_empty() {
                    if let Some(m) = v_as_number(&args[0](ctx, target)) {
                        if m != 0.0 {
                            return V::Double(n % m);
                        }
                    }
                    return V::Double(0.0);
                }
                if name == "div" && !args.is_empty() {
                    if let Some(m) = v_as_number(&args[0](ctx, target)) {
                        if m != 0.0 {
                            return V::Double((n / m).floor());
                        }
                    }
                    return V::Double(0.0);
                }
            }

            V::Null // unmatched method -> invalid (null)
        })
    }
}

/// super-type names for `oclIsKindOf` (DynamicEObject metadata; global registry).
fn super_type_names(obj: &dyn EObject) -> Vec<String> {
    if let Some(dyn_obj) = obj.as_any().downcast_ref::<emf_ecore::DynamicEObject>() {
        let class = dyn_obj.class();
        return class.e_all_super_types(&emf_ecore::ecore_package::global());
    }
    Vec::new()
}

// extension helper: map `self` (This) to the target ref for object ops
trait TargetAccess {
    fn target_some<'a>(&self, target: &'a dyn EObject) -> Option<&'a dyn EObject>;
}
impl TargetAccess for EvalCtx {
    fn target_some<'a>(&self, target: &'a dyn EObject) -> Option<&'a dyn EObject> {
        Some(target)
    }
}

// ===================== public API =====================

/// A `value`-capable evaluator: `Fn(&dyn EObject, Option<Val>) -> bool`.
pub type ValueEvaluator = dyn Fn(&dyn EObject, Option<Val>) -> bool;

/// Compile an expression into a `value`-capable evaluator. The `value` binding
/// (used by constraints like `value > 5`) resolves against the passed-in
/// `Option<Val>`. Parse failure / empty expression returns a constant-true
/// evaluator (tolerant, mirroring the C++/Java constraint-syntax fallback).
pub fn compile_value(expr: &str) -> Box<ValueEvaluator> {
    let toks = tokenize(expr);
    let mut parser = Parser::new(toks);
    let e = parser.parse_top();
    if !parser.ok {
        return Box::new(move |_, _| true);
    }
    Box::new(move |target, value| {
        let mut ctx = EvalCtx::new(value);
        to_bool(&e(&mut ctx, target))
    })
}

/// Compile an expression into an [`Evaluator`] (`Fn(&dyn EObject) -> bool`),
/// compatible with [`crate::constraint::Constraint`]. No external `value` is
/// available here, so any `value` reference resolves to null.
pub fn compile(expr: &str) -> Box<Evaluator> {
    let ev = compile_value(expr);
    Box::new(move |target| ev(target, None))
}

/// Parse an expression into a `Constraint` (mirror of the C++ `parse` helper).
pub fn parse(
    _source: &str,
    name: &str,
    expr: &str,
    severity: Severity,
) -> Constraint {
    Constraint::new(
        compile(expr),
        name, // id mirrors the C++ construction (name-based)
        name,
        format!("constraint '{name}' failed"),
        severity,
        ConstraintMode::Batch,
    )
}

/// Convenience: parse and register an expression constraint into a validator,
/// returning the newly registered constraint (mirror of C++
/// `registerConstraintFromString`).
pub fn register_constraint_from_string(
    validator: &mut EValidator,
    source: &str,
    name: &str,
    expr: &str,
    severity: Severity,
) -> Constraint {
    let registered = parse(source, name, expr, severity);
    let id = registered.id().to_string();
    // register a matching constraint; EValidator replaces any same-id entry
    validator.register_constraint(parse(source, name, expr, severity));
    let _ = id;
    registered
}