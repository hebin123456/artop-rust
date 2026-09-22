//! `ConstraintDescriptor` / `ConstraintCategory` (port of C++ `emf-validation`
//! `ConstraintDescriptor`, aligned to Java
//! `org.eclipse.emf.validation.service.IConstraintDescriptor` and
//! `org.eclipse.emf.validation.model.ConstraintCategory`).
//!
//! A descriptor describes a constraint's metadata + body (the evaluation
//! source), from which an executable [`Constraint`] can be instantiated, plus a
//! small XML parser for the standard `plugin.xml`-style constraint subset.

use crate::constraint::{Constraint, ConstraintMode, Severity};

/// A constraint category (grouping), with a `\``/``\``-separated path.
#[derive(Debug, Clone, Default)]
pub struct ConstraintCategory {
    id: String,
    name: String,
    path: String,
}

impl ConstraintCategory {
    /// New category.
    pub fn new(id: impl Into<String>, name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
        }
    }
    /// Category id.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Category name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Category path (`/a/b`).
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// A constraint descriptor: metadata + body, compiled to a [`Constraint`].
#[derive(Debug, Clone)]
pub struct ConstraintDescriptor {
    id: String,
    name: String,
    description: String,
    message: String,
    severity: Severity,
    mode: ConstraintMode,
    body: String,
    language: String,
    category_path: String,
    code: i32,
}

impl Default for ConstraintDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            message: String::new(),
            severity: Severity::Warning,
            mode: ConstraintMode::Batch,
            body: String::new(),
            language: "expr".to_string(),
            category_path: String::new(),
            code: 0,
        }
    }
}

impl ConstraintDescriptor {
    /// Descriptor id.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Set id.
    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = id.into();
    }
    /// Name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    /// Description.
    pub fn description(&self) -> &str {
        &self.description
    }
    /// Set description.
    pub fn set_description(&mut self, d: impl Into<String>) {
        self.description = d.into();
    }
    /// Message.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Set message.
    pub fn set_message(&mut self, m: impl Into<String>) {
        self.message = m.into();
    }
    /// Severity.
    pub fn severity(&self) -> Severity {
        self.severity
    }
    /// Set severity.
    pub fn set_severity(&mut self, s: Severity) {
        self.severity = s;
    }
    /// Mode.
    pub fn mode(&self) -> ConstraintMode {
        self.mode
    }
    /// Set mode.
    pub fn set_mode(&mut self, m: ConstraintMode) {
        self.mode = m;
    }
    /// Body (evaluation source).
    pub fn body(&self) -> &str {
        &self.body
    }
    /// Set body.
    pub fn set_body(&mut self, b: impl Into<String>) {
        self.body = b.into();
    }
    /// Language key.
    pub fn language(&self) -> &str {
        &self.language
    }
    /// Set language.
    pub fn set_language(&mut self, l: impl Into<String>) {
        self.language = l.into();
    }
    /// Category path.
    pub fn category_path(&self) -> &str {
        &self.category_path
    }
    /// Set category path.
    pub fn set_category_path(&mut self, p: impl Into<String>) {
        self.category_path = p.into();
    }
    /// Status code.
    pub fn code(&self) -> i32 {
        self.code
    }
    /// Set status code.
    pub fn set_code(&mut self, c: i32) {
        self.code = c;
    }

    /// Compile the descriptor into an executable [`Constraint`].
    ///
    /// The body is interpreted as a simple "feature operator matched_value"
    /// expression: the constraint fails when the target's feature value does
    /// not satisfy the predicate. Restricting the parser keeps `emf-validation`
    /// dependency-free; the body grammar is documented next to the expression
    /// helper.
    pub fn instantiate(&self) -> Constraint {
        let body = self.body.clone();
        let id = self.id.clone();
        let name = self.name.clone();
        let message = self.message.clone();
        let severity = self.severity;
        let mode = self.mode;

        // Rudimentary evaluator: "name matches <value>" via OCL-ish equality.
        let evaluator: Box<crate::constraint::Evaluator> = Box::new(move |o| {
            let trimmed = body.trim();
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.is_empty() {
                return true;
            }
            // Support "feature ?= 'literal'" style: split on "==".
            if let Some(idx) = trimmed.find("==") {
                let feat = trimmed[..idx].trim();
                let want = trimmed[idx + 2..].trim().trim_matches('\'').to_string();
                let got = o.e_get(feat).and_then(|v| v.as_str().map(String::from));
                return got.as_deref() == Some(want.as_str());
            }
            if let Some(idx) = trimmed.find("~=") {
                let feat = trimmed[..idx].trim();
                let want = trimmed[idx + 3..]
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();
                let got = o.e_get(feat).and_then(|v| v.as_str().map(String::from));
                let pass = got.map(|g| g.contains(&want)).unwrap_or(false);
                return !pass; // ~= means "must NOT contain"
            }
            let _ = parts;
            true
        });

        Constraint::new(evaluator, id, name, message, severity, mode)
    }
}

/// Descriptor parser: parse an XML string into [`ConstraintDescriptor`]s.
///
/// Expected subset (mirrors the standard `plugin.xml` constraint form):
/// ```xml
/// <constraints>
///   <constraint id=".." name=".." message=".." severity="warning|info|error"
///               mode="live|batch" language="expr" body=".." code="0"
///               categoryPath="/a/b" description=".."/>
/// </constraints>
/// ```
pub struct ConstraintDescriptorParser;

impl ConstraintDescriptorParser {
    /// Parse XML text into descriptors. Returns empty on malformed input.
    pub fn parse_descriptors(xml: &str) -> Vec<ConstraintDescriptor> {
        let mut out = Vec::new();
        let bytes = xml.as_bytes();
        let mut i = 0usize;
        while let Some(start) = find_sub(bytes, i, "<constraint") {
            // find matching '>' or self-closing '/>' honoring quotes
            let (tag_end, close) = find_tag_end(bytes, start);
            let tag = &xml[start..start + "<constraint".len()];
            if !(close == '>' || close == '/') {
                break;
            }
            let attrs = &xml[start + "<constraint".len()..tag_end];
            if let Some(d) = parse_attrs(attrs, tag.starts_with("<constraint")) {
                out.push(d);
            }
            i = tag_end + 1;
            if close == '/' {
                i += 1; // consume the trailing '>'
            }
        }
        out
    }

    /// Parse and register into a validator. Returns number registered.
    pub fn parse_and_register(
        xml: &str,
        validator: &mut crate::e_validator::EValidator,
        prefix: &str,
    ) -> usize {
        let mut n = 0;
        for d in Self::parse_descriptors(xml) {
            if prefix.is_empty() || d.category_path().starts_with(prefix) {
                validator.register_constraint(d.instantiate());
                n += 1;
            }
        }
        n
    }
}

/// Case-insensitive semantics not needed; helper finds a byte substring.
fn find_sub(hay: &[u8], from: usize, needle: &str) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    let n = needle.as_bytes();
    hay[from..]
        .windows(n.len())
        .position(|w| w == n)
        .map(|p| p + from)
}

/// Find the attribute section end and the closing char (`>` or self-closing).
fn find_tag_end(hay: &[u8], start: usize) -> (usize, char) {
    let mut i = start;
    let mut in_quote = false;
    while i < hay.len() {
        let c = hay[i] as char;
        if in_quote {
            if c == '"' {
                in_quote = false;
            }
        } else if c == '"' {
            in_quote = true;
        } else if c == '>' {
            return (i, '>');
        } else if c == '/' && i + 1 < hay.len() && hay[i + 1] as char == '>' {
            return (i, '/');
        }
        i += 1;
    }
    (hay.len(), '>')
}

/// Parse the attribute list of a constraint element.
fn parse_attrs(s: &str, is_constraint: bool) -> Option<ConstraintDescriptor> {
    if !is_constraint {
        return None;
    }
    let mut d = ConstraintDescriptor::default();
    let mut chars = s.char_indices().peekable();
    let mut skipped = 0usize; // unparsed prefix (self-closing `/>` tail not included)
    let mut ok = true;
    // Scan name="value" pairs.
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // skip spaces
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] as char == '/' {
            break;
        }
        let name_start = i;
        while i < bytes.len() && (bytes[i] as char) != '=' {
            i += 1;
        }
        if i >= bytes.len() {
            ok = false;
            break;
        }
        let name = s[name_start..i].trim().to_string();
        i += 1; // '='
                // skip spaces
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] as char != '"' {
            ok = false;
            break;
        }
        i += 1; // opening quote
        let val_start = i;
        while i < bytes.len() && bytes[i] as char != '"' {
            i += 1;
        }
        if i >= bytes.len() {
            ok = false;
            break;
        }
        let value = s[val_start..i].to_string();
        i += 1; // closing quote
        apply_attr(&mut d, &name, &value);
    }
    let _ = (&mut chars, &mut skipped);
    if !ok {
        return None;
    }
    Some(d)
}

/// Apply one parsed attribute to a descriptor.
fn apply_attr(d: &mut ConstraintDescriptor, name: &str, value: &str) {
    match name {
        "id" => d.set_id(value),
        "name" => d.set_name(value),
        "description" => d.set_description(value),
        "message" => d.set_message(value),
        "body" => d.set_body(value),
        "language" => d.set_language(value),
        "categoryPath" => d.set_category_path(value),
        "code" => d.set_code(value.parse().unwrap_or(0)),
        "severity" => d.set_severity(match value {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "info" => Severity::Info,
            "cancel" => Severity::Cancel,
            _ => Severity::Warning,
        }),
        "mode" => d.set_mode(if value == "live" {
            ConstraintMode::Live
        } else {
            ConstraintMode::Batch
        }),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_constraint() {
        let xml = r#"<constraints>
            <constraint id="a" name="A" message="m" severity="error"
                        mode="batch" body="name == 'x'" categoryPath="/cat"/>
        </constraints>"#;
        let ds = ConstraintDescriptorParser::parse_descriptors(xml);
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].id(), "a");
        assert_eq!(ds[0].severity(), Severity::Error);
        assert_eq!(ds[0].category_path(), "/cat");
        assert_eq!(ds[0].body(), "name == 'x'");
    }

    #[test]
    fn instantiate_constraint_evaluates_equality() {
        let mut d = ConstraintDescriptor::default();
        d.set_id("eq");
        d.set_body("name == 'ok'");
        let c = d.instantiate();
        let obj = crate::test_util::make_item();
        // unset name -> feature absent -> fails (Some != "ok")
        assert!(!c.evaluate(&*obj.borrow()));
    }
}
