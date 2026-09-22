//! Diagnostics, ported from C++ `emf-common/Diagnostic` (aligned to Java
//! `org.eclipse.emf.common.util.Diagnostic` / `BasicDiagnostic` / `DiagnosticChain`).

/// Severity of a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Nothing wrong.
    Ok,
    /// Informational.
    Info,
    /// A potential problem.
    Warning,
    /// A definite problem.
    Error,
    /// Cancelled.
    Cancel,
}

impl Severity {
    /// Human name.
    pub fn name(&self) -> &'static str {
        match self {
            Severity::Ok => "OK",
            Severity::Info => "INFO",
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
            Severity::Cancel => "CANCEL",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A single diagnostic message.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    severity: Severity,
    source: String,
    code: i32,
    message: String,
    /// Data payloads (type-erased descriptions).
    data: Vec<String>,
    /// Nested child diagnostics.
    children: Vec<Diagnostic>,
}

impl Diagnostic {
    /// New diagnostic.
    pub fn new(
        severity: Severity,
        source: impl Into<String>,
        code: i32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            source: source.into(),
            code,
            message: message.into(),
            data: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Severity.
    pub fn severity(&self) -> Severity {
        self.severity
    }
    /// Source (plugin/module id).
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Code.
    pub fn code(&self) -> i32 {
        self.code
    }
    /// Message.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Nested children.
    pub fn children(&self) -> &[Diagnostic] {
        &self.children
    }

    /// Add a child diagnostic, cascading its severity upward.
    pub fn add(&mut self, child: Diagnostic) {
        if child.severity > self.severity {
            self.severity = child.severity;
        }
        self.children.push(child);
    }

    /// Merge all children of `other`, cascading severity.
    pub fn add_all(&mut self, other: &Diagnostic) {
        for c in &other.children {
            self.children.push(c.clone());
        }
        if other.severity > self.severity {
            self.severity = other.severity;
        }
    }

    /// An error shorthand.
    pub fn error(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, source, 0, message)
    }

    /// A warning shorthand.
    pub fn warning(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, source, 0, message)
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} {}", self.severity, self.source, self.message)
    }
}

/// A collector of diagnostics (EMF `DiagnosticChain`).
#[derive(Debug, Clone, Default)]
pub struct DiagnosticChain {
    items: Vec<Diagnostic>,
}

impl DiagnosticChain {
    /// New empty chain.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add a diagnostic.
    pub fn add(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of diagnostics.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// The diagnostics.
    pub fn get(&self) -> &[Diagnostic] {
        &self.items
    }

    /// The highest severity present.
    pub fn worst(&self) -> Option<Severity> {
        self.items.iter().map(|d| d.severity).max()
    }

    /// Clear all.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_cascade() {
        let mut root = Diagnostic::new(Severity::Ok, "src", 0, "root");
        assert_eq!(root.severity(), Severity::Ok);
        root.add(Diagnostic::warning("src", "warn"));
        assert_eq!(root.severity(), Severity::Warning);
        root.add(Diagnostic::error("src", "err"));
        assert_eq!(root.severity(), Severity::Error);
        assert_eq!(root.children().len(), 2);
    }

    #[test]
    fn chain_collects() {
        let mut ch = DiagnosticChain::new();
        assert!(ch.is_empty());
        ch.add(Diagnostic::error("src", "boom"));
        ch.add(Diagnostic::warning("src", "warn"));
        assert_eq!(ch.len(), 2);
        assert_eq!(ch.worst(), Some(Severity::Error));
    }
}
