//! `EAnnotation` — a lightweight annotation attached to a model element.
//!
//! Port of C++ `emf-ecore` `EAnnotation` (aligned to Java
//! `org.eclipse.emf.ecore.EAnnotation`). An annotation carries a `source` URI
//! and an ordered list of `(key, value)` detail strings. It is used, among
//! other things, by `emf-validation` to embed OCL / named-constraint
//! expressions directly in an `EClass` (see
//! `emf_validation::annotation_constraint_loader`).

/// A lightweight key/value annotation on a model element (EMF `EAnnotation`).
///
/// The source string identifies the annotation's "schema" (for example
/// `http://www.eclipse.org/emf/2002/Ecore/OCL`), and `details` holds the
/// annotation's keyed string data (EMF `getDetails()`).
#[derive(Debug, Clone, Default)]
pub struct EAnnotation {
    /// The annotation source URI (EMF `EAnnotation.source`).
    source: String,
    /// Order-preserving key/value details (EMF `getDetails()`).
    details: Vec<(String, String)>,
}

impl EAnnotation {
    /// New annotation with the given `source` and no details.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            details: Vec::new(),
        }
    }

    /// The annotation source URI.
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Set the annotation source URI.
    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    /// The key/value details (EMF `getDetails()`), in insertion order.
    pub fn details(&self) -> &[(String, String)] {
        &self.details
    }
    /// Mutable key/value details (EMF `getDetails()`).
    pub fn details_mut(&mut self) -> &mut Vec<(String, String)> {
        &mut self.details
    }

    /// Set a detail entry (EMF `EAnnotation.setDetail`): replaces the entry
    /// for an existing key, or appends a new one.
    pub fn set_detail(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(slot) = self.details.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.details.push((key, value));
        }
    }

    /// Append a detail entry without deduping the key.
    pub fn add_detail(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.details.push((key.into(), value.into()));
    }
}