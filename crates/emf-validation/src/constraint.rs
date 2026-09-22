//! `Constraint` / `IConstraintListener` (port of C++ `emf-validation`
//! `Constraint`, aligned to Java `org.eclipse.emf.validation.model`
//! `IConstraintConstraint` / `IConstraintListener`).
//!
//! A constraint holds an evaluator so an `EObject` returns `true` when it
//! passes, `false` when it is violated, along with identity metadata (id /
//! name / message), severity and evaluation mode. Optional clientContext-style
//! filtering limits the constraint to matching class names.

use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;

/// Constraint severity (EMF `Severity` mirror).
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

/// Evaluation mode (`IValidationConstraint.LIVE` / `BATCH`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintMode {
    /// Live (react to object changes).
    Live,
    /// Batch.
    Batch,
}

/// Evaluator signature: given an `EObject`, returns `true` = pass.
pub type Evaluator = dyn Fn(&dyn EObject) -> bool;

/// A single validation constraint.
pub struct Constraint {
    evaluator: Box<Evaluator>,
    id: String,
    name: String,
    message: String,
    severity: Severity,
    mode: ConstraintMode,
    /// Class-name substring filter (dynamic models). Empty = applies to all.
    target_class_names: Vec<String>,
}

impl Constraint {
    /// New constraint.
    pub fn new(
        evaluator: Box<Evaluator>,
        id: impl Into<String>,
        name: impl Into<String>,
        message: impl Into<String>,
        severity: Severity,
        mode: ConstraintMode,
    ) -> Self {
        Self {
            evaluator,
            id: id.into(),
            name: name.into(),
            message: message.into(),
            severity,
            mode,
            target_class_names: Vec::new(),
        }
    }

    /// Constraint id.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Constraint name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Violation message.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Severity.
    pub fn severity(&self) -> Severity {
        self.severity
    }
    /// Mode.
    pub fn mode(&self) -> ConstraintMode {
        self.mode
    }

    /// Whether scoped to the given class name (dynamic clientContext).
    pub fn applies_to_class(&self, class: &str) -> bool {
        if self.target_class_names.is_empty() {
            return true;
        }
        self.target_class_names.iter().any(|n| class.contains(n))
    }

    /// Add a class-name filter.
    pub fn add_target_class_name(&mut self, n: impl Into<String>) {
        self.target_class_names.push(n.into());
    }

    /// Scoped class-name filters.
    pub fn target_class_names(&self) -> &[String] {
        &self.target_class_names
    }

    /// Evaluate the constraint over `target`. Returns `true` when it passes
    /// (or when the constraint is not applicable).
    pub fn evaluate(&self, target: &dyn EObject) -> bool {
        if !self.applies_to_class(target.e_class()) {
            return true;
        }
        (self.evaluator)(target)
    }
}

/// A listener to constraint registration/unregistration events.
pub trait IConstraintListener {
    /// A constraint was registered.
    fn constraint_registered(&mut self, constraint: &Constraint);
    /// A constraint was unregistered.
    fn constraint_unregistered(&mut self, constraint: &Constraint);
}

/// Object handle used by validator clients.
pub type ObjectHandle = ObjectRef;

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::value::Val;

    #[test]
    fn constraint_evaluates_and_passes() {
        // A constraint that passes when a name feature holds a value >= 2 chars.
        let c = Constraint::new(
            Box::new(|o| {
                let name = o.e_get("name").and_then(|v| v.as_str().map(String::from));
                name.map(|n| n.len() >= 2).unwrap_or(true)
            }),
            "min_length",
            "Min Length",
            "name is too short",
            Severity::Warning,
            ConstraintMode::Batch,
        );
        let item = crate::test_util::make_item();
        assert!(c.evaluate(&*item.borrow()));
        item.borrow_mut().e_set("name", Val::string("ok"));
        assert!(c.evaluate(&*item.borrow()));
        item.borrow_mut().e_set("name", Val::string("x"));
        assert!(!c.evaluate(&*item.borrow()));
    }

    #[test]
    fn class_filter_gates_evaluation() {
        let mut c = Constraint::new(
            Box::new(|_| false),
            "only_item",
            "Only Item",
            "m",
            Severity::Error,
            ConstraintMode::Batch,
        );
        // Filtering on "Other" means an Item object is skipped (passes).
        c.add_target_class_name("Other");
        let item = crate::test_util::make_item();
        assert!(c.evaluate(&*item.borrow()));
    }
}
