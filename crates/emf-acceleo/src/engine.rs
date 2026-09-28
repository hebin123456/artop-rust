//! Acceleo template evaluation engine (port of C++
//! `emf-acceleo/AcceleoEngine.cpp`, aligned to Java
//! `org.eclipse.acceleo.engine.generation.AcceleoEngine` +
//! `org.eclipse.acceleo.engine.service.AcceleoService`).
//!
//! Responsibilities:
//! 1. Take a parsed [`Module`] AST.
//! 2. Evaluate [`Template`]s into text.
//! 3. Support `[file]` blocks that write files (with `[protected]` region merge).
//! 4. Evaluate the AQL expression sub-set against `EObject` reflection.
//! 5. Call registered services, module queries and cross-template calls
//!    (including across an `extends` chain).
//!
//! Expression evaluation is type-erased through [`Val`] (the Rust replacement
//! for C++ `std::any`); navigation (`c.name`) goes through the `EObject`
//! reflection surface (`e_get` by feature name), so the engine is metamodel
//! agnostic.

use crate::ast::*;
use emf_common::value::{ObjectRef, Val};
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A registered template service. Mirrors C++ `ServiceFn`: given the evaluated
/// call arguments and the current context, returns a value.
pub type ServiceFn = Box<dyn Fn(&[Val], &EvalContext) -> Val>;

/// Evaluation context: a flat variable scope. Entering `for`/`let`/query/lambda
/// scopes clones the context, so child writes never leak to the parent — the
/// observable behaviour of C++'s parent-linked `EvalContext`.
#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    vars: HashMap<String, Val>,
}

impl EvalContext {
    /// An empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind (or overwrite) a variable.
    pub fn set(&mut self, name: impl Into<String>, value: Val) {
        self.vars.insert(name.into(), value);
    }

    /// Look up a variable.
    pub fn lookup(&self, name: &str) -> Option<&Val> {
        self.vars.get(name)
    }
}

/// An error raised by the high-level [`AcceleoService`] entry points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    /// Human-readable message.
    pub message: String,
}

impl EngineError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EngineError {}

/// The template evaluation engine.
#[derive(Default)]
pub struct AcceleoEngine {
    services: HashMap<String, ServiceFn>,
    queries: HashMap<String, Rc<Query>>,
    registered_modules: HashMap<String, Rc<Module>>,
    current_module: Option<Rc<Module>>,
    out_dir: Option<PathBuf>,
}

impl std::fmt::Debug for AcceleoEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceleoEngine")
            .field("services", &self.services.keys().collect::<Vec<_>>())
            .field("queries", &self.queries.keys().collect::<Vec<_>>())
            .field(
                "registered_modules",
                &self.registered_modules.keys().collect::<Vec<_>>(),
            )
            .field("out_dir", &self.out_dir)
            .finish()
    }
}

impl AcceleoEngine {
    /// A new engine with no services / modules registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a C++/Rust service callable from templates as `[name(args)/]`.
    pub fn register_service(&mut self, name: impl Into<String>, f: ServiceFn) {
        self.services.insert(name.into(), f);
    }

    /// Whether a service is registered.
    pub fn has_service(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    /// Bind a module's queries so templates can call `queryName(args)`.
    pub fn set_module_queries(&mut self, queries: &[Rc<Query>]) {
        self.queries.clear();
        for q in queries {
            self.queries.insert(q.name.clone(), Rc::clone(q));
        }
    }

    /// Look up a module-level query by name, walking the `extends` chain.
    pub fn lookup_query(&self, name: &str) -> Option<Rc<Query>> {
        if let Some(q) = self.queries.get(name) {
            return Some(Rc::clone(q));
        }
        if let Some(cur) = &self.current_module {
            for parent_name in &cur.extends {
                if let Some(m) = self.registered_modules.get(parent_name) {
                    if let Some(q) = m.query(name) {
                        return Some(q);
                    }
                }
            }
        }
        None
    }

    /// Register a module reachable through the `extends`/`import` chain.
    pub fn register_module(&mut self, m: Rc<Module>) {
        self.registered_modules.insert(m.name.clone(), m);
    }

    /// Set the current main module (establishes the `extends` lookup context and
    /// binds its queries).
    pub fn set_current_module(&mut self, m: Rc<Module>) {
        self.queries.clear();
        for q in &m.queries {
            self.queries.insert(q.name.clone(), Rc::clone(q));
        }
        self.current_module = Some(m);
    }

    /// Look up a template by name: first the current module, then its
    /// `extends` chain.
    pub fn lookup_template(&self, name: &str) -> Option<Rc<Template>> {
        if let Some(cur) = &self.current_module {
            if let Some(t) = cur.template(name) {
                return Some(t);
            }
            for parent_name in &cur.extends {
                if let Some(m) = self.registered_modules.get(parent_name) {
                    if let Some(t) = m.template(name) {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    /// Evaluate a whole template with positional args, returning the generated
    /// text (aligned to Java `AcceleoEngine.evaluate(Template, args)`).
    pub fn evaluate(&self, tpl: &Template, args: &[Val]) -> String {
        let mut ctx = EvalContext::new();
        for (i, p) in tpl.params.iter().enumerate() {
            if let Some(v) = args.get(i) {
                ctx.set(p.name.clone(), v.clone());
            }
        }
        if ctx.lookup("self").is_none() && !args.is_empty() {
            ctx.set("self", args[0].clone());
        }
        let mut out = String::new();
        self.eval_blocks(&tpl.body, &mut ctx, &mut out);
        out
    }

    /// Evaluate every template of `module`, letting `[file]` blocks write under
    /// `out_dir` (aligned to Java `AcceleoService.doGenerate`).
    pub fn do_generate(
        &mut self,
        module: &Rc<Module>,
        model: Option<ObjectRef>,
        out_dir: impl AsRef<Path>,
    ) -> std::io::Result<()> {
        let dir = out_dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        self.out_dir = Some(dir);
        self.set_current_module(Rc::clone(module));
        for tpl in &module.templates {
            let args: Vec<Val> = model
                .clone()
                .map(|m| vec![Val::Object(m)])
                .unwrap_or_default();
            self.evaluate(tpl, &args);
        }
        Ok(())
    }

    // ===== block evaluation =====

    fn eval_blocks(&self, blocks: &[Rc<Block>], ctx: &mut EvalContext, out: &mut String) {
        for b in blocks {
            self.eval_block(b, ctx, out);
        }
    }

    fn eval_block(&self, block: &Block, ctx: &mut EvalContext, out: &mut String) {
        match block {
            Block::Text(t) => out.push_str(t),
            Block::Expr(e) => out.push_str(&value_to_string(&self.eval_expr(e, ctx))),
            Block::For(f) => {
                let col = object_list(&self.eval_expr(&f.collection, ctx));
                let n = col.len();
                for (i, elem) in col.into_iter().enumerate() {
                    let mut child = ctx.clone();
                    child.set(f.var_name.clone(), Val::Object(elem));
                    let mut body = String::new();
                    self.eval_blocks(&f.body, &mut child, &mut body);
                    out.push_str(&body);
                    if f.has_separator && i + 1 < n {
                        out.push_str(&f.separator);
                    }
                }
            }
            Block::If(b) => {
                let c = self.eval_expr(&b.cond, ctx);
                if if_truthy(&c) {
                    self.eval_blocks(&b.then_body, ctx, out);
                } else {
                    let mut done = false;
                    for (cond, body) in &b.else_ifs {
                        let cc = self.eval_expr(cond, ctx);
                        if elseif_truthy(&cc) {
                            self.eval_blocks(body, ctx, out);
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        self.eval_blocks(&b.else_body, ctx, out);
                    }
                }
            }
            Block::Let(b) => {
                let v = self.eval_expr(&b.value, ctx);
                let mut child = ctx.clone();
                child.set(b.var_name.clone(), v);
                self.eval_blocks(&b.body, &mut child, out);
            }
            Block::File(f) => {
                let mut body = String::new();
                let mut child = ctx.clone();
                self.eval_blocks(&f.body, &mut child, &mut body);
                let path = value_to_string(&self.eval_expr(&f.path, ctx));
                if !path.is_empty() {
                    if let Some(dir) = &self.out_dir {
                        // I/O failures are swallowed, mirroring the C++ engine
                        // (which only checks the stream for success).
                        let _ = write_file(dir, &path, f.append, &body);
                    }
                }
            }
            Block::Protected(p) => {
                let id = if p.id.is_empty() {
                    "default".to_string()
                } else {
                    p.id.clone()
                };
                out.push_str("// BEGIN Begin Protected Region ID[");
                out.push_str(&id);
                out.push_str("]\n");
                let mut body = String::new();
                self.eval_blocks(&p.body, ctx, &mut body);
                out.push_str(&body);
                out.push_str("// END End Protected Region ID[");
                out.push_str(&id);
                out.push_str("]\n");
            }
        }
    }

    // ===== expression evaluation =====

    fn eval_expr(&self, e: &Expr, ctx: &mut EvalContext) -> Val {
        match e {
            Expr::Var(name) => {
                if name == "self" || name == "Self" {
                    return ctx.lookup("self").cloned().unwrap_or(Val::Null);
                }
                ctx.lookup(name).cloned().unwrap_or(Val::Null)
            }
            Expr::StringLit(s) => Val::String(s.clone()),
            Expr::IntLit(i) => Val::Int(*i),
            Expr::BoolLit(b) => Val::Bool(*b),
            Expr::Nav { target, name } => {
                let t = self.eval_expr(target, ctx);
                if let Val::Object(o) = &t {
                    o.borrow().e_get(name).unwrap_or(Val::Null)
                } else {
                    Val::Null
                }
            }
            Expr::Call {
                target,
                name,
                args,
                arrow,
            } => self.eval_call(target, name, args, *arrow, ctx),
            Expr::CollectionLit(elems) => {
                Val::List(elems.iter().map(|el| self.eval_expr(el, ctx)).collect())
            }
            Expr::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let c = self.eval_expr(cond, ctx);
                if if_truthy(&c) {
                    self.eval_expr(then_expr, ctx)
                } else {
                    self.eval_expr(else_expr, ctx)
                }
            }
            // A bare lambda never evaluates on its own.
            Expr::Lambda { .. } => Val::Null,
        }
    }

    fn eval_call(
        &self,
        target: &Option<Rc<Expr>>,
        name: &str,
        args: &[Rc<Expr>],
        arrow: bool,
        ctx: &mut EvalContext,
    ) -> Val {
        let target_val = match target {
            Some(t) => self.eval_expr(t, ctx),
            None => Val::Null,
        };

        if arrow {
            match name {
                "size" => {
                    if let Val::String(s) = &target_val {
                        return Val::Int(s.len() as i64);
                    }
                    return Val::Int(object_list(&target_val).len() as i64);
                }
                "isEmpty" => {
                    if let Val::String(s) = &target_val {
                        return Val::Bool(s.is_empty());
                    }
                    return Val::Bool(object_list(&target_val).is_empty());
                }
                "first" | "last" => {
                    let col = object_list(&target_val);
                    if col.is_empty() {
                        return Val::Null;
                    }
                    let o = if name == "first" {
                        col.first().cloned()
                    } else {
                        col.last().cloned()
                    };
                    return o.map(Val::Object).unwrap_or(Val::Null);
                }
                "collect" | "select" | "reject" | "forAll" | "exists" => {
                    return self.eval_collection_op(name, target_val, args, ctx);
                }
                _ => {}
            }
        }

        // Binary / logical operators: the parser encodes `==`, `!=`, `and`,
        // `or` as `Call { target: None, args: [left, right] }`, and `+`/`-` as
        // `Call { target: left, args: [right] }`.
        match name {
            "==" | "!=" => {
                let l = args
                    .first()
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                let r = args
                    .get(1)
                    .map(|a| self.eval_expr(a, ctx))
                    .or_else(|| target.as_ref().map(|t| self.eval_expr(t, ctx)))
                    .unwrap_or(Val::Null);
                let eq = values_equal(&l, &r);
                return Val::Bool(if name == "==" { eq } else { !eq });
            }
            "and" => {
                let l = args
                    .first()
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                if !boolish(&l) {
                    return Val::Bool(false);
                }
                let r = args
                    .get(1)
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                return Val::Bool(boolish(&r));
            }
            "or" => {
                let l = args
                    .first()
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                if boolish(&l) {
                    return Val::Bool(true);
                }
                let r = args
                    .get(1)
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                return Val::Bool(boolish(&r));
            }
            "+" if target.is_some() => {
                let l = self.eval_expr(target.as_ref().unwrap(), ctx);
                let r = args
                    .first()
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                if matches!(l, Val::String(_)) || matches!(r, Val::String(_)) {
                    return Val::String(format!("{}{}", value_to_string(&l), value_to_string(&r)));
                }
                if let (Val::Int(a), Val::Int(b)) = (&l, &r) {
                    return Val::Int(a + b);
                }
                return Val::String(format!("{}{}", value_to_string(&l), value_to_string(&r)));
            }
            "-" if target.is_some() => {
                let l = self.eval_expr(target.as_ref().unwrap(), ctx);
                let r = args
                    .first()
                    .map(|a| self.eval_expr(a, ctx))
                    .unwrap_or(Val::Null);
                if let (Val::Int(a), Val::Int(b)) = (&l, &r) {
                    return Val::Int(a - b);
                }
                return Val::Int(0);
            }
            _ => {}
        }

        // Registered service call.
        if let Some(f) = self.services.get(name) {
            let vals: Vec<Val> = args.iter().map(|a| self.eval_expr(a, ctx)).collect();
            return f(&vals, ctx);
        }

        // Module-level query call (global, no target).
        if target.is_none() {
            if let Some(q) = self.lookup_query(name) {
                let vals: Vec<Val> = args.iter().map(|a| self.eval_expr(a, ctx)).collect();
                let mut child = ctx.clone();
                for (i, p) in q.params.iter().enumerate() {
                    if let Some(v) = vals.get(i) {
                        child.set(p.name.clone(), v.clone());
                    }
                }
                return self.eval_expr(&q.body, &mut child);
            }
        }

        // Cross-template call (global, no target): render and splice the text.
        if target.is_none() {
            if let Some(tpl) = self.lookup_template(name) {
                let vals: Vec<Val> = args.iter().map(|a| self.eval_expr(a, ctx)).collect();
                return Val::String(self.evaluate(&tpl, &vals));
            }
        }

        Val::Null
    }

    /// Evaluate the lambda-driven collection operations
    /// (`collect`/`select`/`reject`/`forAll`/`exists`).
    fn eval_collection_op(
        &self,
        name: &str,
        target_val: Val,
        args: &[Rc<Expr>],
        ctx: &mut EvalContext,
    ) -> Val {
        let col = object_list(&target_val);
        let lambda = args.first().and_then(|a| match &**a {
            Expr::Lambda { var_name, body } => Some((var_name.clone(), Rc::clone(body))),
            _ => None,
        });
        let Some((var_name, body)) = lambda else {
            // No lambda argument: degrade like the C++ engine.
            if name == "forAll" {
                return Val::Bool(true);
            }
            if name == "exists" {
                return Val::Bool(false);
            }
            return Val::List(col.into_iter().map(Val::Object).collect());
        };

        match name {
            "collect" => {
                let mut out = Vec::new();
                for elem in col {
                    let mut child = ctx.clone();
                    child.set(var_name.clone(), Val::Object(elem));
                    let v = self.eval_expr(&body, &mut child);
                    if !v.is_null() {
                        out.push(v);
                    }
                }
                Val::List(out)
            }
            "select" | "reject" => {
                let keep_true = name == "select";
                let mut out = Vec::new();
                for elem in col {
                    let mut child = ctx.clone();
                    child.set(var_name.clone(), Val::Object(elem.clone()));
                    let v = self.eval_expr(&body, &mut child);
                    if to_bool(&v) == keep_true {
                        out.push(elem);
                    }
                }
                Val::List(out.into_iter().map(Val::Object).collect())
            }
            "forAll" => {
                for elem in col {
                    let mut child = ctx.clone();
                    child.set(var_name.clone(), Val::Object(elem));
                    let v = self.eval_expr(&body, &mut child);
                    if !to_bool(&v) {
                        return Val::Bool(false);
                    }
                }
                Val::Bool(true)
            }
            "exists" => {
                for elem in col {
                    let mut child = ctx.clone();
                    child.set(var_name.clone(), Val::Object(elem));
                    let v = self.eval_expr(&body, &mut child);
                    if to_bool(&v) {
                        return Val::Bool(true);
                    }
                }
                Val::Bool(false)
            }
            _ => Val::Null,
        }
    }
}

/// High-level API, aligned to Java
/// `org.eclipse.acceleo.engine.service.AcceleoService`.
#[derive(Default)]
pub struct AcceleoService {
    engine: AcceleoEngine,
}

impl AcceleoService {
    /// A new service wrapping a fresh engine.
    pub fn new() -> Self {
        Self {
            engine: AcceleoEngine::new(),
        }
    }

    /// Register a service on the underlying engine.
    pub fn register_service(&mut self, name: impl Into<String>, f: ServiceFn) {
        self.engine.register_service(name, f);
    }

    /// Shared access to the engine (register modules, etc.).
    pub fn engine(&self) -> &AcceleoEngine {
        &self.engine
    }

    /// Mutable access to the engine.
    pub fn engine_mut(&mut self) -> &mut AcceleoEngine {
        &mut self.engine
    }

    /// Parse `mtl_source`, then evaluate `template_name` with `args` and return
    /// the generated text (no files written).
    pub fn evaluate_template(
        &mut self,
        mtl_source: &str,
        template_name: &str,
        args: &[Val],
    ) -> Result<String, EngineError> {
        let module = crate::parser::parse(mtl_source).map_err(|e| EngineError::new(e.message))?;
        self.engine.set_current_module(Rc::clone(&module));
        match module.template(template_name) {
            Some(tpl) => Ok(self.engine.evaluate(&tpl, args)),
            None => Err(EngineError::new(format!(
                "template not found: {template_name}"
            ))),
        }
    }

    /// Parse `mtl_source`, then generate: evaluate every template, writing
    /// `[file]` blocks under `out_dir`.
    pub fn do_generate(
        &mut self,
        mtl_source: &str,
        model: Option<ObjectRef>,
        out_dir: impl AsRef<Path>,
    ) -> Result<(), EngineError> {
        let module = crate::parser::parse(mtl_source).map_err(|e| EngineError::new(e.message))?;
        self.engine.set_current_module(Rc::clone(&module));
        self.engine
            .do_generate(&module, model, out_dir)
            .map_err(|e| EngineError::new(e.to_string()))
    }
}

// ===== value helpers (mirror the C++ anonymous-namespace helpers) =====

/// Extract the object elements of a list-valued `Val` (C++ `asEObjectVector`).
fn object_list(v: &Val) -> Vec<ObjectRef> {
    match v {
        Val::List(items) => items
            .iter()
            .filter_map(|x| x.as_object().cloned())
            .collect(),
        _ => Vec::new(),
    }
}

/// AQL truthiness (C++ `toBool`): used by `select`/`reject`/`forAll`/`exists`.
fn to_bool(v: &Val) -> bool {
    match v {
        Val::Null => false,
        Val::Bool(b) => *b,
        Val::String(s) => !s.is_empty(),
        Val::Int(i) => *i != 0,
        Val::Byte(b) => *b != 0,
        Val::Double(d) => *d != 0.0,
        Val::EnumLiteral(_) => true,
        Val::Object(_) => true,
        // A collection has no scalar truthiness in the C++ engine.
        Val::List(_) => false,
    }
}

/// `if`-block / conditional-expression truthiness (C++ `IfBlock`/`IfExpr`).
fn if_truthy(v: &Val) -> bool {
    match v {
        Val::Null => false,
        Val::Bool(b) => *b,
        Val::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `elseif` truthiness (C++ checks only bool then "has value").
fn elseif_truthy(v: &Val) -> bool {
    match v {
        Val::Null => false,
        Val::Bool(b) => *b,
        _ => true,
    }
}

/// `and`/`or` operand truthiness (C++: bool else "has value").
fn boolish(v: &Val) -> bool {
    match v {
        Val::Null => false,
        Val::Bool(b) => *b,
        _ => true,
    }
}

/// Equality for `==`/`!=` (mirrors the C++ `CallExpr` comparison ladder).
fn values_equal(l: &Val, r: &Val) -> bool {
    match (l, r) {
        (Val::String(a), Val::String(b)) => a == b,
        (Val::EnumLiteral(a), Val::EnumLiteral(b)) => a == b,
        (Val::Object(a), Val::Object(b)) => Rc::ptr_eq(a, b),
        (Val::Bool(a), Val::Bool(b)) => a == b,
        (Val::Int(a), Val::Int(b)) => a == b,
        (Val::Byte(a), Val::Byte(b)) => a == b,
        (Val::Null, Val::Null) => true,
        _ => false,
    }
}

/// `Val` → string (C++ `anyToString`). Objects render as their `name` feature
/// (or `<EObject>`); lists render as `Sequence{a, b, c}`.
fn value_to_string(v: &Val) -> String {
    match v {
        Val::Null => String::new(),
        Val::String(s) => s.clone(),
        Val::EnumLiteral(s) => s.clone(),
        Val::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Val::Int(i) => i.to_string(),
        Val::Byte(b) => b.to_string(),
        Val::Double(d) => format!("{d}"),
        Val::Object(o) => eobject_to_string(o),
        Val::List(items) => {
            let mut s = String::from("Sequence{");
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&value_to_string(x));
            }
            s.push('}');
            s
        }
    }
}

/// An `EObject`'s textual form: its `name` feature when it is a string, else
/// `<EObject>` (mirrors the C++ `anyToString` EObject branches).
fn eobject_to_string(o: &ObjectRef) -> String {
    match o.borrow().e_get("name") {
        Some(Val::String(s)) => s,
        _ => "<EObject>".to_string(),
    }
}

// ===== file writing / protected-region merge =====

/// Write `body` to `dir/rel`. In append mode the content is appended verbatim;
/// otherwise, when the target already exists, protected regions from the old
/// file are merged over the freshly generated content.
fn write_file(dir: &Path, rel: &str, append: bool, body: &str) -> std::io::Result<()> {
    let full = dir.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    if append {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&full)?;
        f.write_all(body.as_bytes())?;
    } else {
        let merged = if full.exists() {
            match fs::read_to_string(&full) {
                Ok(old) => merge_protected_regions(body, &old),
                Err(_) => body.to_string(),
            }
        } else {
            body.to_string()
        };
        fs::write(&full, merged)?;
    }
    Ok(())
}

/// Merge protected regions: for every `ID[id]` BEGIN/END region in
/// `new_content`, if `old_content` has a region with the same id, substitute
/// the old (user-edited) content. Mirrors the C++ `mergeProtectedRegions`, which
/// implements the Java Acceleo protected-area semantics.
pub fn merge_protected_regions(new_content: &str, old_content: &str) -> String {
    const BEGIN_PREFIX: &str = "BEGIN Begin Protected Region ID[";
    const END_PREFIX: &str = "END End Protected Region ID[";

    // 1. Collect id -> region-body from the old content.
    let mut old_regions: HashMap<String, String> = HashMap::new();
    let mut search_from = 0usize;
    while let Some(b) = old_content[search_from..].find(BEGIN_PREFIX) {
        let b = search_from + b;
        let id_start = b + BEGIN_PREFIX.len();
        let Some(id_end_rel) = old_content[id_start..].find(']') else {
            break;
        };
        let id_end = id_start + id_end_rel;
        let id = &old_content[id_start..id_end];
        let mut content_start = id_end + 1;
        if content_start < old_content.len() && old_content.as_bytes()[content_start] == b'\n' {
            content_start += 1;
        } else if content_start + 1 < old_content.len()
            && old_content.as_bytes()[content_start] == b'\r'
            && old_content.as_bytes()[content_start + 1] == b'\n'
        {
            content_start += 2;
        }
        let end_marker = format!("{END_PREFIX}{id}]");
        let Some(e_rel) = old_content[content_start..].find(&end_marker) else {
            search_from = id_end + 1;
            continue;
        };
        let e = content_start + e_rel;
        let mut content_end = e;
        if content_end > 0 && old_content.as_bytes()[content_end - 1] == b'\n' {
            content_end -= 1;
        }
        if content_end > 0 && old_content.as_bytes()[content_end - 1] == b'\r' {
            content_end -= 1;
        }
        old_regions.insert(
            id.to_string(),
            old_content[content_start..content_end].to_string(),
        );
        search_from = e + end_marker.len();
    }

    if old_regions.is_empty() {
        return new_content.to_string();
    }

    // 2. Rebuild the new content, substituting old region bodies where they exist.
    let mut result = String::new();
    let mut pos = 0usize;
    while pos < new_content.len() {
        let Some(b_rel) = new_content[pos..].find(BEGIN_PREFIX) else {
            result.push_str(&new_content[pos..]);
            break;
        };
        let b = pos + b_rel;
        result.push_str(&new_content[pos..b]);
        let id_start = b + BEGIN_PREFIX.len();
        let Some(id_end_rel) = new_content[id_start..].find(']') else {
            result.push_str(&new_content[b..]);
            break;
        };
        let id_end = id_start + id_end_rel;
        let id = &new_content[id_start..id_end];
        let end_marker = format!("{END_PREFIX}{id}]");
        let Some(e_rel) = new_content[id_end..].find(&end_marker) else {
            result.push_str(&new_content[b..]);
            break;
        };
        let e = id_end + e_rel;

        // Emit the BEGIN marker line.
        let begin_line_end = match new_content[b..].find('\n') {
            Some(nl) if b + nl <= e => b + nl,
            _ => id_end,
        };
        result.push_str(&new_content[b..=begin_line_end]);

        // Emit the region body: prefer the old (user-edited) content.
        if let Some(old) = old_regions.get(id) {
            result.push_str(old);
            if !old.is_empty() && !old.ends_with('\n') {
                result.push('\n');
            }
        } else {
            let content_start = begin_line_end + 1;
            let mut content_end = e;
            if content_end > 0 && new_content.as_bytes()[content_end - 1] == b'\n' {
                content_end -= 1;
            }
            result.push_str(&new_content[content_start..content_end]);
            if content_end > content_start && new_content.as_bytes()[content_end] != b'\n' {
                result.push('\n');
            }
        }

        // Emit the END marker line.
        let Some(end_line_rel) = new_content[e..].find('\n') else {
            result.push_str(&new_content[e..]);
            break;
        };
        let end_line_end = e + end_line_rel;
        result.push_str(&new_content[e..=end_line_end]);
        pos = end_line_end + 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_scopes_are_isolated_when_cloned() {
        let mut parent = EvalContext::new();
        parent.set("a", Val::Int(1));
        let mut child = parent.clone();
        child.set("a", Val::Int(2));
        child.set("b", Val::Int(3));
        assert_eq!(parent.lookup("a"), Some(&Val::Int(1)));
        assert_eq!(child.lookup("a"), Some(&Val::Int(2)));
        assert!(parent.lookup("b").is_none());
    }

    #[test]
    fn evaluates_static_template() {
        let src = "[module t()][template public f()]Hello World[/template][/module]";
        let mut svc = AcceleoService::new();
        assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "Hello World");
    }

    #[test]
    fn evaluates_let_and_if() {
        let src = "[module t()][template public f()][let x = 'hi'][x/][/let][/template][/module]";
        let mut svc = AcceleoService::new();
        assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "hi");

        let src2 =
            "[module t()][template public f()][if (true)]YES[else]NO[/if][/template][/module]";
        assert_eq!(svc.evaluate_template(src2, "f", &[]).unwrap(), "YES");
    }

    #[test]
    fn merges_protected_regions() {
        let new = "head\n// BEGIN Begin Protected Region ID[body]\n// generated\n// END End Protected Region ID[body]\ntail\n";
        let old = "head\n// BEGIN Begin Protected Region ID[body]\n// user edit\n// END End Protected Region ID[body]\ntail\n";
        let merged = merge_protected_regions(new, old);
        assert!(merged.contains("// user edit"));
        assert!(!merged.contains("// generated"));
    }
}
