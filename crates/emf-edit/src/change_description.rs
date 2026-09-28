//! `ChangeDescription` / `FeatureChange` — the set of changes a command applies
//! (port of C++ `emf-edit` `ChangeDescription`, aligned to Java
//! `org.eclipse.emf.edit.command.ChangeDescription`).
//!
//! A [`FeatureChange`] records one object/feature old→new transition; a
//! [`ChangeDescription`] holds a list of them and can `apply` (write the new
//! values through the reflection surface) or `apply_and_reverse` (apply, then
//! swap each change so the description becomes the undo description).
//!
//! `None` for a value means "no value" (EMF `std::any{}`): applying it unsets
//! the feature rather than writing a null.

use emf_common::value::{ObjectRef, Val};
use emf_ecore::EStructuralFeature;

/// A single feature change (C++ `FeatureChange`).
#[derive(Debug, Clone, Default)]
pub struct FeatureChange {
    /// The object whose feature changes.
    pub e_object: Option<ObjectRef>,
    /// The structural feature being changed.
    pub feature: Option<EStructuralFeature>,
    /// The value before the change (`None` = no value).
    pub old_value: Option<Val>,
    /// The value after the change (`None` = unset).
    pub new_value: Option<Val>,
}

/// A collection of feature changes (C++ `ChangeDescription`).
#[derive(Debug, Clone, Default)]
pub struct ChangeDescription {
    changes: Vec<FeatureChange>,
}

impl ChangeDescription {
    /// An empty change description.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a change (C++ `add`).
    pub fn add(&mut self, change: FeatureChange) {
        self.changes.push(change);
    }

    /// The recorded changes (C++ `getChanges`).
    pub fn changes(&self) -> &[FeatureChange] {
        &self.changes
    }

    /// Whether no changes are recorded (C++ `isEmpty`).
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Drop all recorded changes (C++ `clear`).
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Apply every change through the reflection surface (C++ `apply`).
    /// A `None` new value unsets the feature.
    pub fn apply(&self) {
        for c in &self.changes {
            let (Some(obj), Some(feature)) = (&c.e_object, &c.feature) else {
                continue;
            };
            let name = feature.name();
            let mut o = obj.borrow_mut();
            match &c.new_value {
                Some(v) => {
                    let _ = o.e_set(name, v.clone());
                }
                None => {
                    let _ = o.e_unset(name);
                }
            }
        }
    }

    /// Apply every change, then swap each change's old/new value so this
    /// description now describes the reverse (C++ `applyAndReverse`). Returns a
    /// copy holding the reversed changes.
    pub fn apply_and_reverse(&mut self) -> ChangeDescription {
        self.apply();
        for c in &mut self.changes {
            std::mem::swap(&mut c.old_value, &mut c.new_value);
        }
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_description() {
        let cd = ChangeDescription::new();
        assert!(cd.is_empty());
        assert_eq!(cd.changes().len(), 0);
    }

    #[test]
    fn add_then_clear() {
        let mut cd = ChangeDescription::new();
        cd.add(FeatureChange::default());
        assert!(!cd.is_empty());
        assert_eq!(cd.changes().len(), 1);
        cd.clear();
        assert!(cd.is_empty());
    }
}
