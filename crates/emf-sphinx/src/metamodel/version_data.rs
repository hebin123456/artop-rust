//! `MetaModelVersionData` — version-scoped metadata of a meta-model.
//!
//! Port of C++ `emf/sphinx/metamodel/MetaModelVersionData.h` (aligned to Java
//! `org.eclipse.sphinx.emf.metamodel.MetaModelVersionData`).
//!
//! C++ keeps a raw `IMetaModelDescriptor*` for the optional base descriptor;
//! Rust uses a shared [`Rc<dyn MetaModelDescriptor>`] handle instead.

use std::rc::Rc;

use super::descriptor::MetaModelDescriptor;

/// Version data identifying one concrete version of a meta-model.
#[derive(Clone, Default)]
pub struct MetaModelVersionData {
    ns_postfix: String,
    e_package_ns_uri_postfix_pattern: String,
    name: String,
    base_descriptor: Option<Rc<dyn MetaModelDescriptor>>,
    ordinal: i32,
}

impl std::fmt::Debug for MetaModelVersionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetaModelVersionData")
            .field("ns_postfix", &self.ns_postfix)
            .field(
                "e_package_ns_uri_postfix_pattern",
                &self.e_package_ns_uri_postfix_pattern,
            )
            .field("name", &self.name)
            .field("has_base_descriptor", &self.base_descriptor.is_some())
            .field("ordinal", &self.ordinal)
            .finish()
    }
}

impl MetaModelVersionData {
    /// Three-arg constructor: `ns_postfix / ePackageNsURIPostfixPattern / name`.
    /// The ordinal defaults to `-1` (no explicit ordering).
    pub fn new(
        ns_postfix: impl Into<String>,
        e_package_ns_uri_postfix_pattern: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            ns_postfix: ns_postfix.into(),
            e_package_ns_uri_postfix_pattern: e_package_ns_uri_postfix_pattern.into(),
            name: name.into(),
            base_descriptor: None,
            ordinal: -1,
        }
    }

    /// Four-arg constructor, additionally carrying an explicit ordinal.
    pub fn with_ordinal(
        ns_postfix: impl Into<String>,
        e_package_ns_uri_postfix_pattern: impl Into<String>,
        name: impl Into<String>,
        ordinal: i32,
    ) -> Self {
        Self {
            ordinal,
            ..Self::new(ns_postfix, e_package_ns_uri_postfix_pattern, name)
        }
    }

    /// Five-arg constructor, additionally carrying a base descriptor.
    pub fn with_base_descriptor(
        ns_postfix: impl Into<String>,
        e_package_ns_uri_postfix_pattern: impl Into<String>,
        name: impl Into<String>,
        base_descriptor: Rc<dyn MetaModelDescriptor>,
    ) -> Self {
        Self {
            base_descriptor: Some(base_descriptor),
            ..Self::new(ns_postfix, e_package_ns_uri_postfix_pattern, name)
        }
    }

    /// The namespace postfix, e.g. `v1`.
    pub fn ns_postfix(&self) -> &str {
        &self.ns_postfix
    }

    /// The EPackage namespace-URI postfix pattern, e.g. `v[0-9]+`.
    pub fn e_package_ns_uri_postfix_pattern(&self) -> &str {
        &self.e_package_ns_uri_postfix_pattern
    }

    /// The display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The base descriptor, if any.
    pub fn base_descriptor(&self) -> Option<&Rc<dyn MetaModelDescriptor>> {
        self.base_descriptor.as_ref()
    }

    /// The ordinal (`-1` when unset).
    pub fn ordinal(&self) -> i32 {
        self.ordinal
    }

    /// Set the namespace postfix.
    pub fn set_ns_postfix(&mut self, v: impl Into<String>) {
        self.ns_postfix = v.into();
    }

    /// Set the EPackage namespace-URI postfix pattern.
    pub fn set_e_package_ns_uri_postfix_pattern(&mut self, v: impl Into<String>) {
        self.e_package_ns_uri_postfix_pattern = v.into();
    }

    /// Set the display name.
    pub fn set_name(&mut self, v: impl Into<String>) {
        self.name = v.into();
    }

    /// Set the base descriptor.
    pub fn set_base_descriptor(&mut self, d: Rc<dyn MetaModelDescriptor>) {
        self.base_descriptor = Some(d);
    }

    /// Set the ordinal.
    pub fn set_ordinal(&mut self, v: i32) {
        self.ordinal = v;
    }

    /// Structural equality over every field (aligned to C++
    /// `MetaModelVersionData::equals`). The base descriptor is compared by
    /// pointer identity, like the C++ raw-pointer comparison.
    pub fn equals(&self, other: &MetaModelVersionData) -> bool {
        self.ns_postfix == other.ns_postfix
            && self.e_package_ns_uri_postfix_pattern == other.e_package_ns_uri_postfix_pattern
            && self.name == other.name
            && self.ordinal == other.ordinal
            && match (&self.base_descriptor, &other.base_descriptor) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
    }
}
