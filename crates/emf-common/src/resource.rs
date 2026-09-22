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

    /// Set the single root object (EMF `Resource.setRoot`).
    pub fn set_root(&mut self, obj: ObjectRef) {
        self.contents = vec![obj];
        self.modified = true;
    }

    /// Append a root object.
    pub fn add_to_contents(&mut self, obj: ObjectRef) {
        self.contents.push(obj);
        self.modified = true;
    }

    /// Replace the whole root contents.
    pub fn set_contents(&mut self, contents: Vec<ObjectRef>) {
        self.contents = contents;
        self.modified = true;
    }

    /// Remove all root contents.
    pub fn clear_contents(&mut self) {
        self.contents.clear();
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

    // ==== persistence surface (aligned to C++ `Resource::save/load`) ====

    /// Serialize the resource to an XMI/XML string. The base implementation is
    /// a no-op (empty); a concrete serializing resource overrides this.
    pub fn save_to_string(&self) -> String {
        String::new()
    }

    /// `toXmiString` — delegate to [`Self::save_to_string`].
    pub fn to_xmi_string(&self) -> String {
        self.save_to_string()
    }

    /// Parse XMI/XML text into this resource's contents. The base
    /// implementation is a no-op and does **not** flip `is_loaded` (aligned to
    /// the C++ base `load(istream)`).
    pub fn load_from_string(&mut self, _src: &str) {}

    /// `fromXmiString` — delegate to [`Self::load_from_string`].
    pub fn from_xmi_string(&mut self, src: &str) {
        self.load_from_string(src);
    }

    /// Resolve an object by URI fragment. Base returns `None` (aligned to the
    /// C++ base `getEObject` returning `nullptr`).
    pub fn get_eobject(&self, _fragment: &str) -> Option<ObjectRef> {
        None
    }

    /// Compute the URI fragment for an object. Base returns empty (aligned to
    /// the C++ base `getURIFragment`).
    pub fn get_uri_fragment(&self, _obj: &ObjectRef) -> String {
        String::new()
    }

    /// Load from the resource's URI. Base tries to read the file for a `file:`
    /// URI and surface an error if it cannot be opened (aligned to the C++
    /// base `load()`).
    pub fn load(&mut self) -> Result<(), String> {
        if self.uri.is_file() {
            let path = self.uri.to_file_path();
            if path.is_empty() {
                return Err(format!("cannot load from empty file path"));
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("Cannot open file: {} ({e})", path))?;
            self.load_from_string(&text);
            self.loaded = true;
            Ok(())
        } else {
            Err(format!(
                "no stream available for scheme '{}'",
                self.uri.scheme()
            ))
        }
    }

    /// Save to the resource's URI. Base tries to write the file for a `file:`
    /// URI and surface an error if the destination cannot be written (aligned
    /// to the C++ base `save()`).
    pub fn save(&mut self) -> Result<(), String> {
        if self.uri.is_file() {
            let path = self.uri.to_file_path();
            if path.is_empty() {
                return Err(format!("cannot save to empty file path"));
            }
            let text = self.to_xmi_string();
            std::fs::write(&path, text)
                .map_err(|e| format!("Cannot write file: {} ({e})", path))?;
            self.modified = false;
            Ok(())
        } else {
            Err(format!(
                "no stream available for scheme '{}'",
                self.uri.scheme()
            ))
        }
    }
}

/// A load/save-capable resource handle. This is the abstract persistence unit a
/// [`ResourceSet`] can create (via a [`ResourceFactory`]) and load on demand.
///
/// `emf-common` stays serializer-agnostic: it defines the *surface* here but
/// never knows how a concrete implementation persists. A serializing crate
/// (e.g. `emf-xmi`'s `XMIResourceFactory`) implements the trait and hands a
/// factory to the set.
pub trait ResourceHandle: std::fmt::Debug {
    /// The resource URI.
    fn uri(&self) -> &Uri;
    /// Whether the resource has been loaded.
    fn is_loaded(&self) -> bool;
    /// Mark the resource loaded/unloaded.
    fn set_loaded(&mut self, loaded: bool);
    /// Root objects.
    fn contents(&self) -> &[ObjectRef];
    /// Replace the root objects.
    fn set_contents(&mut self, contents: Vec<ObjectRef>);
    /// Load contents from the resource URI.
    fn load(&mut self) -> Result<(), String>;
    /// Persist contents to the resource URI.
    fn save(&mut self) -> Result<(), String>;
    /// Downcast to the concrete type.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Mutable downcast to the concrete type.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Creates resources on demand for a [`ResourceSet`]. Mirrors EMF
/// `Resource.Factory` / `Resource.Factory.Registry` (the canonical
/// implementation is `emf-xmi::XMIResourceFactory`); the set falls back to
/// plain in-memory [`Resource`]s when no factory is registered.
pub trait ResourceFactory: std::fmt::Debug {
    /// Build a resource bound to `uri`.
    fn create(&self, uri: Uri) -> Box<dyn ResourceHandle>;
}

impl ResourceHandle for Resource {
    fn uri(&self) -> &Uri {
        &self.uri
    }
    fn is_loaded(&self) -> bool {
        self.loaded
    }
    fn set_loaded(&mut self, loaded: bool) {
        self.loaded = loaded;
    }
    fn contents(&self) -> &[ObjectRef] {
        &self.contents
    }
    fn set_contents(&mut self, contents: Vec<ObjectRef>) {
        self.contents = contents;
        self.modified = true;
    }
    fn load(&mut self) -> Result<(), String> {
        if self.uri.is_file() {
            let path = self.uri.to_file_path();
            if path.is_empty() {
                return Err("cannot load from empty file path".to_string());
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("Cannot open file: {} ({e})", path))?;
            self.load_from_string(&text);
            self.loaded = true;
            Ok(())
        } else {
            Err(format!(
                "no stream available for scheme '{}'",
                self.uri.scheme()
            ))
        }
    }
    fn save(&mut self) -> Result<(), String> {
        if self.uri.is_file() {
            let path = self.uri.to_file_path();
            if path.is_empty() {
                return Err("cannot save to empty file path".to_string());
            }
            let text = self.to_xmi_string();
            std::fs::write(&path, text)
                .map_err(|e| format!("Cannot write file: {} ({e})", path))?;
            self.modified = false;
            Ok(())
        } else {
            Err(format!(
                "no stream available for scheme '{}'",
                self.uri.scheme()
            ))
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A `ResourceSet` owns resources and produces new ones on demand from a
/// [`ResourceFactory`] (EMF `ResourceSet` + `Resource.Factory.Registry`).
#[derive(Debug, Default)]
pub struct ResourceSet {
    resources: Vec<Box<dyn ResourceHandle>>,
    factory: Option<Box<dyn ResourceFactory>>,
}

impl ResourceSet {
    /// New empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the factory used to produce resources for URIs the set does
    /// not already hold (EMF `ResourceSet.setResourceFactoryRegistry`).
    pub fn set_resource_factory(&mut self, factory: Box<dyn ResourceFactory>) {
        self.factory = Some(factory);
    }

    /// The registered factory, if any.
    pub fn resource_factory(&self) -> Option<&dyn ResourceFactory> {
        self.factory.as_deref()
    }

    /// All resources held by the set, in creation order.
    pub fn resources(&self) -> &[Box<dyn ResourceHandle>] {
        &self.resources
    }

    /// EMF `createResource(uri)`: create a resource for `uri` (via the factory,
    /// falling back to an in-memory [`Resource`]) and add it to the set.
    pub fn create_resource(&mut self, uri: Uri) -> &mut Box<dyn ResourceHandle> {
        self.resources.push(self.make(uri));
        self.resources.last_mut().unwrap()
    }

    /// EMF `getResource(uri, loadOnDemand)`: return the resource at `uri`,
    /// loading it first when `load_on_demand` and it is not yet loaded. When
    /// the URI is absent and `load_on_demand`, the factory creates it, adds it
    /// to the set and loads it. A load failure surfaces as `Err`.
    pub fn get_resource(
        &mut self,
        uri: &Uri,
        load_on_demand: bool,
    ) -> Result<Option<&Box<dyn ResourceHandle>>, String> {
        let idx = self.resources.iter().position(|r| r.uri() == uri);
        match idx {
            Some(i) => {
                if load_on_demand && !self.resources[i].is_loaded() {
                    self.resources[i].load()?;
                }
                Ok(Some(&self.resources[i]))
            }
            None => {
                if !load_on_demand {
                    return Ok(None);
                }
                let mut created = self.make(uri.clone());
                created.load()?;
                self.resources.push(created);
                Ok(self.resources.last())
            }
        }
    }

    /// Produce a resource for `uri` via the factory, or a plain in-memory
    /// [`Resource`] when none is registered.
    fn make(&self, uri: Uri) -> Box<dyn ResourceHandle> {
        match &self.factory {
            Some(f) => f.create(uri),
            None => Box::new(Resource::new(uri)),
        }
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

    #[test]
    fn default_resource_state() {
        let r = Resource::new(Uri::default());
        assert!(r.contents().is_empty());
        assert!(r.root().is_none());
        assert!(!r.is_loaded());
        assert!(!r.is_modified());
        assert!(r.errors().is_empty());
        assert!(r.warnings().is_empty());
        assert!(r.uri().to_string().is_empty() || r.uri().to_string() == "");
    }

    #[test]
    fn set_root_returns_same() {
        let mut r = Resource::new(Uri::parse("file:///a.xmi"));
        let rc: ObjectRef = Rc::new(RefCell::new(R {}));
        r.set_root(Rc::clone(&rc));
        assert_eq!(r.contents().len(), 1);
        let got = r.root().expect("root set");
        assert!(Rc::ptr_eq(got, &rc));
    }

    #[test]
    fn set_uri_updates_resource() {
        let mut r = Resource::new(Uri::parse("file:///before.xmi"));
        assert!(r.uri().is_file());
        r.set_uri(Uri::parse("platform:/resource/m/after.xmi"));
        assert!(!r.uri().is_file());
        assert!(r.uri().is_platform());
    }

    #[test]
    fn set_loaded_and_modified_flags() {
        let mut r = Resource::new(Uri::default());
        r.set_loaded(true);
        assert!(r.is_loaded());
        r.set_modified(true);
        assert!(r.is_modified());
        r.set_loaded(false);
        r.set_modified(false);
        assert!(!r.is_loaded());
        assert!(!r.is_modified());
    }

    #[test]
    fn errors_and_warnings_persist() {
        let mut r = Resource::new(Uri::default());
        r.set_errors(vec!["boom".into()]);
        r.set_warnings(vec!["careful".into()]);
        assert_eq!(r.errors(), ["boom"]);
        assert_eq!(r.warnings(), ["careful"]);
    }

    #[test]
    fn resource_set_creates_and_looks_up() {
        let mut set = ResourceSet::new();
        set.create_resource(Uri::parse("file:///r1.xmi"));
        set.create_resource(Uri::parse("file:///r2.xmi"));
        assert_eq!(set.resources().len(), 2);
        assert!(set
            .get_resource(&Uri::parse("file:///r1.xmi"), false)
            .unwrap()
            .is_some());
        assert!(set
            .get_resource(&Uri::parse("file:///nope.xmi"), false)
            .unwrap()
            .is_none());
    }

    /// A handle whose `load()` merely records the call and flips `is_loaded`,
    /// so we can verify factory dispatch and load-on-demand without touching
    /// the file system.
    #[derive(Debug)]
    struct DummyResource {
        uri: Uri,
        loaded: bool,
        contents: Vec<ObjectRef>,
        loads: usize,
    }

    impl ResourceHandle for DummyResource {
        fn uri(&self) -> &Uri {
            &self.uri
        }
        fn is_loaded(&self) -> bool {
            self.loaded
        }
        fn set_loaded(&mut self, loaded: bool) {
            self.loaded = loaded;
        }
        fn contents(&self) -> &[ObjectRef] {
            &self.contents
        }
        fn set_contents(&mut self, contents: Vec<ObjectRef>) {
            self.contents = contents;
        }
        fn load(&mut self) -> Result<(), String> {
            self.loaded = true;
            self.loads += 1;
            Ok(())
        }
        fn save(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[derive(Debug)]
    struct DummyFactory;

    impl ResourceFactory for DummyFactory {
        fn create(&self, uri: Uri) -> Box<dyn ResourceHandle> {
            Box::new(DummyResource {
                uri,
                loaded: false,
                contents: Vec::new(),
                loads: 0,
            })
        }
    }

    #[test]
    fn resource_set_uses_factory_on_create() {
        let mut set = ResourceSet::new();
        set.set_resource_factory(Box::new(DummyFactory));
        let r = set.create_resource(Uri::parse("file:///d.xmi"));
        let dummy = r.as_any().downcast_ref::<DummyResource>().unwrap();
        assert!(!dummy.is_loaded());
    }

    #[test]
    fn resource_set_loads_on_demand_via_factory() {
        let mut set = ResourceSet::new();
        set.set_resource_factory(Box::new(DummyFactory));
        let d = Uri::parse("file:///d.xmi");

        // Absent URI + loadOnDemand => factory creates it and load() runs once.
        {
            let got = set
                .get_resource(&d, true)
                .unwrap()
                .expect("created on demand");
            assert!(got.is_loaded());
        } // release the mutable borrow taken by get_resource
        assert_eq!(set.resources().len(), 1);
        {
            let dummy = set.resources()[0]
                .as_any()
                .downcast_ref::<DummyResource>()
                .unwrap();
            assert_eq!(dummy.loads, 1);
        }

        // Re-reading returns the same resource without a second load.
        {
            let _again = set
                .get_resource(&d, true)
                .unwrap()
                .expect("already present");
        }
        assert_eq!(set.resources().len(), 1);
        {
            let dummy = set.resources()[0]
                .as_any()
                .downcast_ref::<DummyResource>()
                .unwrap();
            assert_eq!(dummy.loads, 1);
        }

        // loadOnDemand=false on an absent URI returns None without creating.
        assert!(set
            .get_resource(&Uri::parse("file:///absent.xmi"), false)
            .unwrap()
            .is_none());
        assert_eq!(set.resources().len(), 1);
    }

    #[test]
    fn save_load_stream_defaults_no_throw() {
        let mut r = Resource::new(Uri::default());
        // base save_to_string is a no-op: empty, no exception.
        assert_eq!(r.save_to_string(), "");
        // base load_from_string is a no-op: does not flip is_loaded.
        r.load_from_string("anything");
        assert!(!r.is_loaded());
    }

    #[test]
    fn to_xmi_string_delegates_save() {
        let r = Resource::new(Uri::default());
        assert_eq!(r.to_xmi_string(), ""); // base save is a no-op
    }

    #[test]
    fn from_xmi_string_delegates_load_no_throw() {
        let mut r = Resource::new(Uri::default());
        r.from_xmi_string("<x/>");
        assert!(!r.is_loaded());
    }

    #[test]
    fn get_eobject_empty_fragment_is_none() {
        let r = Resource::new(Uri::default());
        assert!(r.get_eobject("").is_none());
        assert!(r.get_eobject("//_1").is_none());
    }

    #[test]
    fn get_uri_fragment_returns_empty() {
        let r = Resource::new(Uri::default());
        let rc: ObjectRef = Rc::new(RefCell::new(R {}));
        assert_eq!(r.get_uri_fragment(&rc), "");
    }

    #[test]
    fn load_nonexistent_file_throws() {
        let mut r = Resource::new(Uri::parse("file:///nonexistent/path/does/not/exist.mi"));
        assert!(r.load().is_err());
    }

    #[test]
    fn save_nonexistent_file_throws() {
        let mut r = Resource::new(Uri::parse("file:///nonexistent/path/does/not/exist.mi"));
        assert!(r.save().is_err());
    }
}
