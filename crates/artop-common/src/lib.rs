//! # artop-common
//!
//! Foundational EMF infrastructure, ported from the C++ `emf-common` module of
//! `hebin123456/artop-cpp`. Mirrors the Eclipse EMF core concepts: `EObject`,
//! `EList`, `Resource`, `URI`, `Diagnostic`, `EPackageRegistry`, `EList`,
//! `Command`, and the notification/feature-map primitives.
//!
//! This is the bottom of the dependency graph; every other crate builds on it.

pub mod diagnostic {
    //! Severity levels for diagnostics (C++ `Diagnostic.h` boundary).

    /// Severity of a diagnostic message.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Severity {
        /// Informational.
        Info,
        /// A potential problem.
        Warning,
        /// A definite problem.
        Error,
        /// A fatal problem.
        Fatal,
    }
}

pub mod diag {
    //! Minimal reporting sink; will grow into the `Diagnostic` subsystem.
    use super::diagnostic::Severity;

    /// Emit a diagnostic line. Placeholder until the full `Diagnostic` port lands.
    pub fn report(sev: Severity, msg: &str) {
        eprintln!("[{sev:?}] {msg}");
    }
}

pub mod uri {
    //! URI value type (C++ `URI`).

    /// A URI as an owned string.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Uri(String);

    impl Uri {
        /// Build a new URI.
        pub fn new(value: impl Into<String>) -> Self {
            Self(value.into())
        }

        /// The URI as `&str`.
        pub fn value(&self) -> &str {
            &self.0
        }
    }

    impl std::fmt::Display for Uri {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }
}

pub mod elist {
    //! EMF `EList` abstraction and a basic implementation (C++ `EList.*`).

    /// Ordered list interface corresponding to EMF's `EList`.
    pub trait EList<T> {
        /// Number of elements.
        fn len(&self) -> usize;
        /// Whether the list is empty.
        fn is_empty(&self) -> bool {
            self.len() == 0
        }
        /// Element at position `i`.
        fn get(&self, i: usize) -> Option<&T>;
        /// Append `v`.
        fn push(&mut self, v: T);
        /// Remove the element at `i`.
        fn remove(&mut self, i: usize) -> Option<T>;
    }

    /// Standard growable list backed by `Vec`.
    #[derive(Debug, Clone)]
    pub struct BasicEList<T>(Vec<T>);

    impl<T> Default for BasicEList<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> BasicEList<T> {
        /// Empty list.
        pub fn new() -> Self {
            Self(Vec::new())
        }

        /// Raw access to the backing vector.
        pub fn as_slice(&self) -> &[T] {
            &self.0
        }
    }

    impl<T> EList<T> for BasicEList<T> {
        fn len(&self) -> usize {
            self.0.len()
        }

        fn get(&self, i: usize) -> Option<&T> {
            self.0.get(i)
        }

        fn push(&mut self, v: T) {
            self.0.push(v);
        }

        fn remove(&mut self, i: usize) -> Option<T> {
            if i < self.0.len() {
                Some(self.0.remove(i))
            } else {
                None
            }
        }
    }

    impl<T> IntoIterator for BasicEList<T> {
        type Item = T;
        type IntoIter = std::vec::IntoIter<T>;
        fn into_iter(self) -> Self::IntoIter {
            self.0.into_iter()
        }
    }
}

pub mod eobject {
    //! The `EObject` trait (C++ `EObject.*` / EMF `EObject`).
    //!
    //! The full EMF `EObject` surface (eClass, eContainer, features, adaptive
    //! observer registration) is layered in as the port matures; here we keep
    //! the minimal contract that the tree of objects in a `Resource` shares.

    /// Base trait for all model objects, mirroring EMF's `EObject`.
    pub trait EObject {
        /// The classifier (class) name of this object, e.g. `"EClass"`.
        fn e_class(&self) -> &'static str;

        /// Structures help debugging and adapting; filled by object ports.
        fn as_any(&self) -> &dyn std::any::Any {
            unimplemented!("adapters: exposed by concrete object types")
        }
    }

    /// Helper to downcast a `&dyn EObject` to a concrete type.
    pub fn downcast<T: 'static>(_obj: &dyn EObject) -> Option<&T> {
        None // to be implemented with `Any` in the object supertrait
    }
}

pub mod resource {
    //! `Resource` abstraction (C++ `Resource.*` / EMF `Resource`).
    use super::uri::Uri;

    /// A model resource addressable by URI that holds root objects.
    pub trait Resource {
        /// URI of this resource.
        fn uri(&self) -> &Uri;
        /// A short type name for the resource.
        fn type_name(&self) -> &'static str;
    }
}

pub mod epackage_registry {
    //! EPackage registry (C++ `EPackageRegistry.*` / EMF `EPackage.Registry`).

    /// Registry binding namespace URIs to `EPackage`s.
    #[derive(Debug, Clone, Default)]
    pub struct EPackageRegistry {
        entries: Vec<(String, String)>,
    }

    impl EPackageRegistry {
        /// New empty registry.
        pub fn new() -> Self {
            Self::default()
        }

        /// Register a package by `ns_uri`.
        pub fn register(&mut self, ns_uri: impl Into<String>, package: impl Into<String>) {
            self.entries.push((ns_uri.into(), package.into()));
        }

        /// Look up a package by `ns_uri`.
        pub fn lookup(&self, ns_uri: &str) -> Option<&str> {
            self.entries
                .iter()
                .find(|(ns, _)| ns == ns_uri)
                .map(|(_, p)| p.as_str())
        }
    }
}

pub mod feature_map {
    //! Feature maps (C++ `FeatureMap.*`, `BasicFeatureMap`) — placeholder.

    /// A typed key for feature-map entries.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FeatureId(pub u16);

    /// Marker: feature-map architecture resumes here.
    pub const FEATURE_MAP_KIND: usize = 0;
}

pub mod enotifier {
    //! Notification / observer model (C++ `ENotifier.*`) — placeholder.

    /// Notification event kinds.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NotificationKind {
        Add,
        Remove,
        Set,
        Move,
    }

    /// Marker: notifier wiring resumes here.
    pub const NOTIFIER_KIND: usize = 0;
}

pub mod uriconverter {
    //! URI conversion (C++ `URIConverter.*`) — placeholder.

    /// Marker for URI-to-path mapping.
    pub const URI_MAP_LOADED: bool = false;
}

pub mod command {
    //! Command framework base (C++ `Command` in `emf-common`) — placeholder.

    /// Marker for the command stack.
    pub const COMMAND_V1: usize = 0;
}

pub mod segment_sequence {
    //! Segment-sequence helper (C++ `SegmentSequence`) — placeholder.

    /// Marker for the segment-set algorithm.
    pub const SEGMENT_EQL: &str = "segment-sequence";
}

#[cfg(test)]
mod tests {
    use super::diagnostic::Severity;
    use super::elist::{BasicEList, EList};
    use super::uri::Uri;

    #[test]
    fn elist_behaves() {
        let mut l = BasicEList::new();
        l.push(1);
        l.push(2);
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(1), Some(&2));
        assert_eq!(l.remove(0), Some(1));
    }

    #[test]
    fn uri_and_severity_roundtrip() {
        let u = Uri::new("file:///x.arxml");
        assert_eq!(u.value(), "file:///x.arxml");
        assert_eq!(Severity::Error, Severity::Error);
    }
}
