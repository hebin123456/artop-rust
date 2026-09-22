//! EMF `Resource` / `ResourceSet`, ported from C++ `emf-common/Resource`
//! (aligned to Java `org.eclipse.emf.ecore.resource.Resource`, `ResourceSet`).
//!
//! A `Resource` owns a URI and a list of root `EObject`s. It is the unit of
//! serialization/deserialization (`save` / `load`), and gives objects a stable
//! home that `e_resource()` resolves up the containment tree.

use crate::uri::Uri;
use crate::value::ObjectRef;

/// A resource: a URI-addressable unit of model content.
#[derive(Debug, Default, Clone)]
pub struct Resource {
    uri: Uri,
    contents: Vec<ObjectRef>,
    loaded: bool,
    modified: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Resource {
    /// New resource at the given URI.
    pub fn new(uri: Uri) -> Self {
        Self {
            uri,
            ..Self::default()
        }
    }

    /// The resource URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Set the resource URI.
    pub fn set_uri(&mut self, uri: Uri) {
        self.uri = uri;
    }

    /// Root contents.
    pub fn contents(&self) -> &[ObjectRef] {
        &self.contents
    }

    /// First root object, if any.
    pub fn root(&self) -> Option<&ObjectRef> {
        self.contents.first()
    }

    /// Append a root object.
    pub fn add_to_contents(&mut self, obj: ObjectRef) {
        self.contents.push(obj);
        self.modified = true;
    }

    /// Whether loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Mark loaded.
    pub fn set_loaded(&mut self, loaded: bool) {
        self.loaded = loaded;
    }

    /// Whether modified.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Mark modified.
    pub fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    /// Errors collected during load/save.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Warnings collected during load/save.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Set the errors buffer.
    pub fn set_errors(&mut self, errors: Vec<String>) {
        self.errors = errors;
    }

    /// Set the warnings buffer.
    pub fn set_warnings(&mut self, warnings: Vec<String>) {
        self.warnings = warnings;
    }
}

/// A `ResourceSet` owns a shared URI converter and a set of resources.
#[derive(Debug, Default)]
pub struct ResourceSet {
    resources: Vec<Resource>,
}

impl ResourceSet {
    /// New empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resource at the URI, adding it to the set.
    pub fn create_resource(&mut self, uri: Uri) -> &mut Resource {
        self.resources.push(Resource::new(uri));
        self.resources.last_mut().unwrap()
    }

    /// All resources.
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }

    /// Look up a resource by URI.
    pub fn get_resource(&self, uri: &Uri) -> Option<&Resource> {
        self.resources.iter().find(|r| &r.uri == uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eobject::EObject;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct R;
    impl std::fmt::Debug for R {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("R")
        }
    }
    impl EObject for R {
        fn e_class(&self) -> &str {
            "Root"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn resource_roundtrip_uri_and_contents() {
        let mut r = Resource::new(Uri::parse("file:///a.xmi"));
        assert!(r.uri().is_file());
        let rc: ObjectRef = Rc::new(RefCell::new(R {}));
        r.add_to_contents(Rc::clone(&rc));
        assert_eq!(r.contents().len(), 1);
        assert_eq!(r.root().unwrap().borrow().e_class(), "Root");
    }
}
