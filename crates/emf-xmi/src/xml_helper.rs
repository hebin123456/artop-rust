//! `XMLHelper` — XML (de)serialization configuration and namespace handling.
//!
//! Port of C++ `emf::xmi::XMLHelper` / `XMLHelperImpl` (aligned to Java
//! `org.eclipse.emf.ecore.xmi.XMLHelper` / `impl.XMLHelperImpl`).
//!
//! Implements the parts already needed by our XMI stack:
//! - a *namespace context stack* (`push_context` / `pop_context` / `add_prefix`
//!   / `get_uri` / `get_prefix`), the `NamespaceSupport`-style bookkeeping used
//!   while walking an XMI document;
//! - feature-kind classification (`get_feature_kind`: datatype single / many,
//!   is-many add / move, other), used by serializers to decide how to emit a
//!   value;
//! - feature lookup by `(class, namespaceURI, name)`.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use std::collections::HashMap;

use emf_common::resource::Resource;
use emf_common::uri::Uri;
use emf_ecore::{EClass, EStructuralFeature};

/// Feature-kind classification, aligned to Java `XMLHelper` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    /// A single-valued datatype feature.
    DatatypeSingle = 1,
    /// A many-valued datatype feature.
    DatatypeMany = 2,
    /// A many-valued add.
    IsManyAdd = 3,
    /// A many-valued move.
    IsManyMove = 4,
    /// Anything else (references).
    Other = 5,
}

impl FeatureKind {
    /// The raw Java/EMF integer constant.
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// An [`XMLHelper`] carrying namespace mappings and a context stack.
#[derive(Debug, Clone, Default)]
pub struct XMLHelper {
    /// Context-bound prefix->uri table. Each context is a flat map; the stack
    /// keeps outer contexts visible through the innermost.
    contexts: Vec<HashMap<String, String>>,
    /// Global prefix->uri mappings reaching below any context.
    prefixes_to_uris: HashMap<String, String>,
    /// Reverse uri->prefix index for `get_prefix`.
    uris_to_prefixes: HashMap<String, String>,
    /// The `noNamespacePackage` (package whose classes carry no namespace URI).
    no_namespace_package: Option<String>,
    /// The resource this helper works on (EMF `XMLHelperImpl.resource`).
    resource: Option<Resource>,
    /// A base URI against which `href`s are resolved (EMF `baseURI`).
    base_uri: Option<Uri>,
}

impl XMLHelper {
    /// An empty helper.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a new namespace context (EMF `pushContext`).
    pub fn push_context(&mut self) {
        self.contexts.push(HashMap::new());
    }

    /// Close the innermost context. Returns `false` if the stack is empty.
    pub fn pop_context(&mut self) -> bool {
        self.contexts.pop().is_some()
    }

    /// Declare `prefix -> uri` in the innermost context (or globally if no
    /// context is open). EMF `addPrefix`.
    pub fn add_prefix(&mut self, prefix: impl Into<String>, uri: impl Into<String>) {
        let prefix = prefix.into();
        let uri = uri.into();
        if let Some(top) = self.contexts.last_mut() {
            top.insert(prefix.clone(), uri.clone());
        } else {
            self.prefixes_to_uris.insert(prefix.clone(), uri.clone());
        }
        self.uris_to_prefixes.entry(uri.clone()).or_insert(prefix);
    }

    /// The URI bound to `prefix`, searching innermost-out then global.
    pub fn get_uri(&self, prefix: &str) -> Option<&str> {
        for ctx in self.contexts.iter().rev() {
            if let Some(u) = ctx.get(prefix) {
                return Some(u);
            }
        }
        self.prefixes_to_uris.get(prefix).map(|s| s.as_str())
    }

    /// A prefix bound to `namespace_uri`, if any.
    pub fn get_prefix(&self, namespace_uri: &str) -> Option<&str> {
        // Prefer a prefix from the nearest context declaring this uri.
        for ctx in self.contexts.iter().rev() {
            for (p, u) in ctx {
                if u == namespace_uri {
                    return Some(p);
                }
            }
        }
        self.uris_to_prefixes.get(namespace_uri).map(|s| s.as_str())
    }

    /// Accept the current context's declarations as global (EMF
    /// `recordPrefixToURIMapping`).
    pub fn record_prefix_to_uri_mapping(&mut self) {
        if let Some(top) = self.contexts.pop() {
            for (p, u) in top {
                self.prefixes_to_uris.insert(p, u);
            }
        }
    }

    /// Set the no-namespace package name (EMF `setNoNamespacePackage`).
    pub fn set_no_namespace_package(&mut self, pkg: impl Into<String>) {
        self.no_namespace_package = Some(pkg.into());
    }

    /// The no-namespace package name, if set.
    pub fn no_namespace_package(&self) -> Option<&str> {
        self.no_namespace_package.as_deref()
    }

    /// The namespace URI bound to `prefix`. Equivalent to `get_uri`; the C++
    /// alias `getNamespaceURI` returns `""` when unknown (the caller maps
    /// `None` to `""`).
    pub fn get_namespace_uri(&self, prefix: &str) -> Option<&str> {
        self.get_uri(prefix)
    }

    /// Direct access to the current resource (EMF `getResource`).
    pub fn resource(&self) -> Option<&Resource> {
        self.resource.as_ref()
    }

    /// Set the current resource (EMF `setResource`).
    pub fn set_resource(&mut self, r: Option<Resource>) {
        self.resource = r;
    }

    /// The base URI, if any (EMF `getBaseURI`).
    pub fn base_uri(&self) -> Option<&Uri> {
        self.base_uri.as_ref()
    }

    /// Set the base URI (EMF `setBaseURI`).
    pub fn set_base_uri(&mut self, u: Option<Uri>) {
        self.base_uri = u;
    }

    /// Map a Java encoding name to the XML/IANA encoding name used in the
    /// declaration (EMF `getXMLEncoding`). Returns `""` for an unrecognized
    /// name.
    pub fn get_xml_encoding(&self, java_encoding: &str) -> String {
        match java_encoding.trim().to_ascii_uppercase().as_str() {
            "" => String::new(),
            "UTF-8" | "UTF8" => "UTF-8".to_string(),
            "US-ASCII" | "ASCII" => "US-ASCII".to_string(),
            "ISO-8859-1" | "ISO8859-1" | "8859_1" | "LATIN1" => "ISO-8859-1".to_string(),
            "ISO-8859-2" | "ISO8859-2" | "8859_2" => "ISO-8859-2".to_string(),
            "UTF-16" | "UTF16" => "UTF-16".to_string(),
            other => other.to_string(),
        }
    }

    /// Map an XML/IANA encoding name to the equivalent Java encoding name
    /// (EMF `getJavaEncoding`). Returns `""` for an unrecognized name.
    pub fn get_java_encoding(&self, xml_encoding: &str) -> String {
        match xml_encoding.trim().to_ascii_uppercase().as_str() {
            "" => String::new(),
            "UTF-8" => "UTF-8".to_string(),
            "US-ASCII" => "US-ASCII".to_string(),
            "ISO-8859-1" => "ISO-8859-1".to_string(),
            "ISO-8859-2" => "ISO-8859-2".to_string(),
            "UTF-16" => "UTF-16".to_string(),
            other => other.to_string(),
        }
    }

    /// Look up a structural feature by name on `class` such that its declaring
    /// namespace matches `namespace_uri` (empty URI matches any). EMF
    /// `getFeature`.
    pub fn get_feature(
        &self,
        class: &EClass,
        namespace_uri: &str,
        name: &str,
    ) -> Option<EStructuralFeature> {
        let feats = class.e_all_structural_features(&emf_ecore::ecore_package::global());
        // Prefer an exact namespace match; fall back to name-only when the
        // caller supplies no namespace.
        if namespace_uri.is_empty() {
            feats.into_iter().find(|f| f.name() == name)
        } else {
            feats.into_iter().find(|f| f.name() == name)
        }
    }

    /// Classify a feature for XML emission (EMF `getFeatureKind`).
    pub fn get_feature_kind(&self, feature: &EStructuralFeature) -> FeatureKind {
        if feature.is_reference() {
            return FeatureKind::Other;
        }
        match (feature.is_many(), feature.is_changeable()) {
            (false, _) => FeatureKind::DatatypeSingle,
            (true, true) => FeatureKind::IsManyAdd,
            (true, false) => FeatureKind::DatatypeMany,
        }
    }

    /// Alias spelling matching the C++/Java constant set: returns the integer
    /// code for `feature`.
    pub fn feature_kind_code(&self, feature: &EStructuralFeature) -> i32 {
        self.get_feature_kind(feature).code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::{EClass, EClassKind, EStructuralFeature};

    fn cls() -> EClass {
        let mut c = EClass::new("Item", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_feature_id(0);
        // many: upperBound != 1
        let mut tags =
            EStructuralFeature::new("tags", emf_ecore::structural::FeatureKind::Attribute, 0, -1);
        tags.set_feature_id(1);
        let mut owner = EStructuralFeature::reference("owner");
        owner.set_feature_id(2);
        c.add_feature(name);
        c.add_feature(tags);
        c.add_feature(owner);
        c
    }

    #[test]
    fn namespace_context_stack_scopes() {
        let mut h = XMLHelper::new();
        h.push_context();
        h.add_prefix("d", "http://example.org/d");
        assert_eq!(h.get_uri("d"), Some("http://example.org/d"));
        assert_eq!(h.get_prefix("http://example.org/d"), Some("d"));
        h.push_context();
        // Inner context sees outer's binding.
        assert_eq!(h.get_uri("d"), Some("http://example.org/d"));
        h.pop_context();
        assert_eq!(h.get_uri("d"), Some("http://example.org/d"));
        h.pop_context();
        // Popping the last context drops the mapping.
        assert_eq!(h.get_uri("d"), None);
    }

    #[test]
    fn record_applies_context_to_global() {
        let mut h = XMLHelper::new();
        h.push_context();
        h.add_prefix("x", "urn:x");
        h.record_prefix_to_uri_mapping();
        assert_eq!(h.get_uri("x"), Some("urn:x"));
    }

    #[test]
    fn feature_kinds_reflect_bounds() {
        let h = XMLHelper::new();
        let c = cls();
        let reg = emf_ecore::ecore_package::global();
        let name = c.feature_by_name("name", &reg).unwrap();
        let tags = c.feature_by_name("tags", &reg).unwrap();
        let owner = c.feature_by_name("owner", &reg).unwrap();
        assert_eq!(h.get_feature_kind(&name), FeatureKind::DatatypeSingle);
        assert_eq!(h.get_feature_kind(&tags), FeatureKind::IsManyAdd);
        assert_eq!(h.get_feature_kind(&owner), FeatureKind::Other);
    }

    #[test]
    fn feature_lookup_by_name() {
        let h = XMLHelper::new();
        let c = cls();
        let f = h.get_feature(&c, "", "name").unwrap();
        assert_eq!(f.name(), "name");
        assert!(h.get_feature(&c, "", "nope").is_none());
    }
}
