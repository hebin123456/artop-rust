//! Concrete XMI-backed resource (port of C++ `emf::xmi::XMIResource`,
//! aligned to Java `org.eclipse.emf.ecore.xmi.impl.XMIResourceImpl`).
//!
//! In artop-cpp the generic `Resource` declares a no-op `save`/`load` and the
//! concrete `XMIResource` overrides them to (de)serialize XMI. Rust has no
//! inheritance, so this module is the concrete counterpart: it wraps an
//! [`emf_common::resource::Resource`] (the abstract URI/contents unit) plus a
//! [`PackageRegistry`], and performs *real* XMI persistence through the
//! [`super::saver`] / [`super::loader`] pair.
//!
//! Being part of the generic EMF XMI layer it has no knowledge of any domain
//! metamodel; the registry drives all class/feature resolution.

use std::rc::Rc;

use emf_common::resource::{Resource, ResourceHandle};
use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::PackageRegistry;

use super::loader::load_from_str_with_ids;
use super::options::XmiOptions;
use super::saver::save_to_string;
use super::xml_save_impl::{XMLLoader, XMLSave, XMLSaveImpl, XMLoaderImpl};

/// An XMI-serializable [`Resource`] bound to a [`PackageRegistry`].
pub struct XMIResource {
    resource: Resource,
    registry: PackageRegistry,
    opts: XmiOptions,
    /// `xmi:id` (or an id set via [`set_id`](Self::set_id)) -> object.
    id_to_eobject: std::collections::HashMap<String, ObjectRef>,
    /// Object identity (pointer) -> id, for the reverse `get_id` lookup.
    eobject_to_id: std::collections::HashMap<usize, String>,
    /// Whether [`ensure_id`](Self::ensure_id) auto-assigns a UUID.
    use_uuids: bool,
    /// Active serializer (defaults to [`XMLSaveImpl`]; injectable).
    xml_save: Rc<dyn XMLSave>,
    /// Active deserializer (defaults to [`XMLoaderImpl`]; injectable).
    xml_load: Rc<dyn XMLLoader>,
}

impl std::fmt::Debug for XMIResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Trait-object handles have no stable Debug, so summarize the durable
        // shape (uri/loaded/id tables) instead of the plugin handles.
        f.debug_struct("XMIResource")
            .field("uri", &self.resource.uri())
            .field("loaded", &self.resource.is_loaded())
            .field("contents", &self.resource.contents().len())
            .field("use_uuids", &self.use_uuids)
            .finish()
    }
}

impl XMIResource {
    /// A new XMI resource at `uri`, resolving its metamodel via `registry`.
    pub fn new(uri: Uri, registry: PackageRegistry) -> Self {
        Self {
            resource: Resource::new(uri),
            registry,
            opts: XmiOptions::default(),
            id_to_eobject: std::collections::HashMap::new(),
            eobject_to_id: std::collections::HashMap::new(),
            use_uuids: false,
            xml_save: Rc::new(XMLSaveImpl),
            xml_load: Rc::new(XMLoaderImpl),
        }
    }

    /// Wrap an existing abstract [`Resource`].
    pub fn from_parts(resource: Resource, registry: PackageRegistry) -> Self {
        Self {
            resource,
            registry,
            opts: XmiOptions::default(),
            id_to_eobject: std::collections::HashMap::new(),
            eobject_to_id: std::collections::HashMap::new(),
            use_uuids: false,
            xml_save: Rc::new(XMLSaveImpl),
            xml_load: Rc::new(XMLoaderImpl),
        }
    }

    /// The serialization options (mutable for customization).
    pub fn options_mut(&mut self) -> &mut XmiOptions {
        &mut self.opts
    }

    /// The serialization options (read-only).
    pub fn options(&self) -> &XmiOptions {
        &self.opts
    }

    /// Access the underlying abstract resource.
    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    /// The registry this resource resolves classes/features against.
    pub fn registry(&self) -> &PackageRegistry {
        &self.registry
    }

    /// Mutable access to the underlying abstract resource.
    pub fn resource_mut(&mut self) -> &mut Resource {
        &mut self.resource
    }

    /// Serialize the current contents to an XMI/XML string, routed through the
    /// active [`XMLSave`] (custom or the default real serializer).
    pub fn save_to_string(&self) -> String {
        self.xml_save.save(self)
    }

    /// The real XMI serializer (used by the default [`XMLSaveImpl`]).
    pub(crate) fn save_inner(&self) -> String {
        save_to_string(self.resource.contents(), &self.opts)
    }

    /// `toXmiString` — delegate to [`Self::save_to_string`].
    pub fn to_xmi_string(&self) -> String {
        self.save_to_string()
    }

    /// Parse XMI/XML text into the resource, routed through the active
    /// [`XMLLoader`] (custom or the default real parser).
    pub fn load_from_string(&mut self, src: &str) -> Result<(), String> {
        let loader = self.xml_load.clone();
        loader.load(self, src)
    }

    /// The real XMI parser into this resource (used by the default
    /// [`XMLoaderImpl`]); replaces roots, marks it loaded, adopts `xmi:id`s.
    pub(crate) fn load_inner(&mut self, src: &str) -> Result<(), String> {
        let (roots, id_map) = load_from_str_with_ids(src, &self.registry)?;
        self.resource.set_contents(roots);
        self.resource.set_loaded(true);
        // Adopt every `xmi:id` discovered while loading into the id map.
        self.id_to_eobject = id_map;
        self.eobject_to_id = self
            .id_to_eobject
            .iter()
            .map(|(id, obj)| (object_key(obj), id.clone()))
            .collect();
        Ok(())
    }

    /// `fromXmiString` — delegate to [`Self::load_from_string`].
    pub fn from_xmi_string(&mut self, src: &str) -> Result<(), String> {
        self.load_from_string(src)
    }

    // ---- UUID generation & auto-ID (EMF `XMIResource.generateUUID/ensureID`)

    /// Whether [`ensure_id`](Self::ensure_id) auto-assigns UUIDs
    /// (EMF `useUUIDs`). Defaults to `false`.
    pub fn use_uuids(&self) -> bool {
        self.use_uuids
    }

    /// Set whether [`ensure_id`](Self::ensure_id) auto-assigns UUIDs.
    pub fn set_use_uuids(&mut self, enabled: bool) {
        self.use_uuids = enabled;
    }

    /// Generate a version-4 UUID string: 36 chars, `8-4-4-4-12`, with the
    /// version nibble `4` at index 14 and variant nibble `{8,9,a,b}` at index
    /// 19 (EMF `XMIResource.generateUUID`).
    pub fn generate_uuid(&self) -> String {
        generate_v4_uuid()
    }

    /// Ensure `obj` has an id. With `useUUIDs` off, returns `""` and does not
    /// touch the id maps; with it on, generates a UUID the first time and
    /// returns the same id on subsequent calls (idempotent, EMF `ensureID`).
    pub fn ensure_id(&mut self, obj: &ObjectRef) -> String {
        if !self.use_uuids {
            return String::new();
        }
        let existing = self.get_id(obj);
        if !existing.is_empty() {
            return existing;
        }
        let id = generate_v4_uuid();
        self.set_id(obj, id.clone());
        id
    }

    // ---- save/load injection (EMF `XMLSave` / `XMLLoad` abstractions) ----

    /// The active [`XMLSave`]; the default [`XMLSaveImpl`] unless replaced.
    /// The returned handle is cached, so repeated calls yield the same
    /// instance (mirrors C++ `getXMLSave` lazy caching).
    pub fn get_xml_save(&self) -> Rc<dyn XMLSave> {
        self.xml_save.clone()
    }

    /// Inject a custom serializer (EMF `setXMLSave`).
    pub fn set_xml_save<S: XMLSave + 'static>(&mut self, save: S) {
        self.xml_save = Rc::new(save);
    }

    /// Inject a custom serializer by shared handle.
    pub fn set_xml_save_rc(&mut self, save: Rc<dyn XMLSave>) {
        self.xml_save = save;
    }

    /// The active [`XMLLoader`]; the default [`XMLoaderImpl`] unless replaced.
    /// Cached so repeated calls yield the same instance.
    pub fn get_xml_load(&self) -> Rc<dyn XMLLoader> {
        self.xml_load.clone()
    }

    /// Inject a custom deserializer (EMF `setXMLLoad`).
    pub fn set_xml_load<L: XMLLoader + 'static>(&mut self, load: L) {
        self.xml_load = Rc::new(load);
    }

    /// Inject a custom deserializer by shared handle.
    pub fn set_xml_load_rc(&mut self, load: Rc<dyn XMLLoader>) {
        self.xml_load = load;
    }

    // ---- ID / href navigation (EMF `XMIResource.getID/getEObject` etc.) ----

    /// Register `id` for `obj`, replacing any prior id (EMF `setID`).
    pub fn set_id(&mut self, obj: &ObjectRef, id: impl Into<String>) {
        let key = object_key(obj);
        if let Some(old) = self.eobject_to_id.get(&key).cloned() {
            self.id_to_eobject.remove(&old);
        }
        let id = id.into();
        self.eobject_to_id.insert(key, id.clone());
        self.id_to_eobject.insert(id, Rc::clone(obj));
    }

    /// The id registered for `obj`, if any (EMF `getID`; `""` when none).
    pub fn get_id(&self, obj: &ObjectRef) -> String {
        self.eobject_to_id
            .get(&object_key(obj))
            .cloned()
            .unwrap_or_default()
    }

    /// The object registered under `id`, if any (EMF `getEObjectByID`).
    pub fn get_object_by_id(&self, id: &str) -> Option<ObjectRef> {
        self.id_to_eobject.get(id).cloned()
    }

    /// All `id -> object` entries currently registered. Aligns the C++
    /// `getIDToEObjectMap` accessor.
    pub fn id_to_eobject_map(&self) -> &std::collections::HashMap<String, ObjectRef> {
        &self.id_to_eobject
    }

    /// Resolve a fragment to an object (EMF `XMIResource.getEObject`).
    ///
    /// Supports the forms tested by the C++ suite:
    /// - `"?<id>"` -> by `xmi:id`
    /// - `"Name"` / `"//Name"` (leading slashes stripped) -> by classifier name
    /// An empty or unknown fragment yields `None`.
    pub fn get_eobject(&self, fragment: &str) -> Option<ObjectRef> {
        if fragment.is_empty() {
            return None;
        }
        if let Some(id) = fragment.strip_prefix('?') {
            return self.get_object_by_id(id);
        }
        let name = fragment.trim_start_matches('/');
        find_object_by_name(self.resource.contents(), name)
    }

    /// Resolve a position path into an object (EMF `resolvePositionPath`).
    ///
    /// `"@feat.0[.feat2.1...]"` walks a containment feature by index from a
    /// root; a path without `@` resolves by classifier name, as `get_eobject`.
    pub fn resolve_position_path(&self, path: &str) -> Option<ObjectRef> {
        if path.is_empty() {
            return None;
        }
        if path.starts_with('@') {
            let segments: Vec<&str> = path[1..].split('.').collect();
            for root in self.resource.contents().iter() {
                if let Some(obj) = navigate_position(root, &segments) {
                    return Some(obj);
                }
            }
            None
        } else {
            find_object_by_name(self.resource.contents(), path.trim_start_matches('/'))
        }
    }

    /// Persist to the resource URI (`file:` only for now).
    pub fn save(&mut self) -> Result<(), String> {
        if !self.resource.uri().is_file() {
            return Err(format!(
                "no stream available for scheme '{}'",
                self.resource.uri().scheme()
            ));
        }
        let path = self.resource.uri().to_file_path();
        if path.is_empty() {
            return Err("cannot save to empty file path".to_string());
        }
        let text = self.to_xmi_string();
        std::fs::write(&path, text).map_err(|e| format!("Cannot write file: {} ({e})", path))?;
        self.resource.set_modified(false);
        Ok(())
    }

    /// Load from the resource URI (`file:` only for now).
    pub fn load(&mut self) -> Result<(), String> {
        if !self.resource.uri().is_file() {
            return Err(format!(
                "no stream available for scheme '{}'",
                self.resource.uri().scheme()
            ));
        }
        let path = self.resource.uri().to_file_path();
        if path.is_empty() {
            return Err("cannot load from empty file path".to_string());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot open file: {} ({e})", path))?;
        self.load_from_string(&text)
    }
}

/// Present an [`XMIResource`] through the abstract [`ResourceHandle`] surface so a
/// [`ResourceSet`] can hold it alongside other resource kinds and load it on
/// demand (EMF `Resource` / `ResourceSet.getResource(uri, loadOnDemand)`).
impl ResourceHandle for XMIResource {
    fn uri(&self) -> &Uri {
        self.resource.uri()
    }
    fn is_loaded(&self) -> bool {
        self.resource.is_loaded()
    }
    fn set_loaded(&mut self, loaded: bool) {
        self.resource.set_loaded(loaded);
    }
    fn contents(&self) -> &[ObjectRef] {
        self.resource.contents()
    }
    fn set_contents(&mut self, contents: Vec<ObjectRef>) {
        self.resource.set_contents(contents);
    }
    fn load(&mut self) -> Result<(), String> {
        self.load()
    }
    fn save(&mut self) -> Result<(), String> {
        self.save()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A stable identity for an [`ObjectRef`] (the underlying `Rc` pointer), used
/// as the reverse `object -> id` map key.
fn object_key(obj: &ObjectRef) -> usize {
    Rc::as_ptr(obj) as *const () as usize
}

/// A monotonic source of per-call uniqueness fused into the UUID, so even two
/// UUIDs generated within the same nanosecond never collide.
static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// SplitMix64 step: turns one state word into a well-mixed 64-bit value.
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Generate a version-4 UUID string (RFC 4122): 36 chars in `8-4-4-4-12`
/// layout, version nibble `4`, variant nibble in `{8,9,a,b}`.
fn generate_v4_uuid() -> String {
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as u64;
    let a = splitmix64(nanos ^ (seq.wrapping_mul(2_654_435_761)));
    let b = splitmix64(seq ^ nanos.rotate_left(17) ^ 0xA24B_AED9_6376_C9C2);

    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant 10xx

    let hex: String = bytes.iter().map(|x| format!("{x:02x}")).collect();
    // Re-assemble with the hyphens: 8-4-4-4-12.
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Depth-first search for the first reachable object (including roots) whose
/// eClass name equals `name`.
fn find_object_by_name(roots: &[ObjectRef], name: &str) -> Option<ObjectRef> {
    fn rec(obj: &ObjectRef, name: &str) -> Option<ObjectRef> {
        if obj.borrow().e_class() == name {
            return Some(Rc::clone(obj));
        }
        for child in obj.borrow().e_contents() {
            if let Some(found) = rec(&child, name) {
                return Some(found);
            }
        }
        None
    }
    for root in roots {
        if let Some(found) = rec(root, name) {
            return Some(found);
        }
    }
    None
}

/// Walk a `@feat.0[.feat2.1...]` position path from `root`. Returns the object
/// at the end of the path, or `None` if any feature/index is missing.
fn navigate_position(root: &ObjectRef, segments: &[&str]) -> Option<ObjectRef> {
    let mut current = Rc::clone(root);
    let mut i = 0;
    while i + 1 < segments.len() {
        let feat = segments[i];
        let index: usize = segments[i + 1].parse().ok()?;
        let value = current.borrow().e_get(feat)?;
        let next = match value {
            Val::List(items) => items.get(index)?.as_object()?.clone(),
            Val::Object(sub) => {
                if index == 0 {
                    sub
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        current = next;
        i += 2;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::eobject::EObject;
    use emf_common::value::Val;
    use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, EStructuralFeature};
    use std::cell::RefCell;
    use std::rc::Rc;

    const NS: &str = "http://example.org/library";

    fn library_reg() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("library");
        pkg.set_ns_prefix("lib");
        pkg.set_ns_uri(NS);

        let mut chapter = EClass::new("Chapter", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        chapter.add_feature(name);

        let mut book = EClass::new("Book", EClassKind::Class);
        let mut title = EStructuralFeature::attribute("title");
        title.set_type_name("EString");
        let mut chapters = EStructuralFeature::reference_many("chapters");
        chapters.set_type_name("Chapter");
        chapters.set_containment(true);
        book.add_feature(title);
        book.add_feature(chapters);

        pkg.add_class(book);
        pkg.add_class(chapter);

        let mut reg = PackageRegistry::new();
        reg.register(make_package_ref(pkg));
        reg
    }

    fn make_book(registry: &PackageRegistry) -> ObjectRef {
        let book_cls = registry.find_class("Book").unwrap();
        let chapter_cls = registry.find_class("Chapter").unwrap();
        let chapter = Rc::new(RefCell::new(DynamicEObject::new_in(
            chapter_cls,
            registry.clone(),
        )));
        chapter
            .borrow_mut()
            .e_set("name", Val::String("Chapter 1".into()));
        let book = Rc::new(RefCell::new(DynamicEObject::new_in(
            book_cls,
            registry.clone(),
        )));
        book.borrow_mut()
            .e_set("title", Val::String("The Library".into()));
        book.borrow_mut()
            .e_set("chapters", Val::List(vec![Val::Object(chapter)]));
        book
    }

    #[test]
    fn save_then_load_roundtrips_contents() {
        let registry = library_reg();
        let mut r = XMIResource::new(Uri::parse("file:///a.xmi"), registry.clone());

        let root = make_book(&registry);
        r.resource_mut().add_to_contents(root);
        r.resource_mut().set_modified(true);

        let xmi = r.save_to_string();
        assert!(xmi.contains("<lib:Book "), "{xmi}");
        assert!(xmi.contains("title=\"The Library\""), "{xmi}");

        // Load into a fresh resource and verify the graph is reconstructed.
        let mut r2 = XMIResource::new(Uri::parse("file:///b.xmi"), registry);
        assert!(!r2.resource().is_loaded());
        r2.load_from_string(&xmi).unwrap();
        assert!(r2.resource().is_loaded());
        let contents = r2.resource().contents();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].borrow().e_class(), "Book");
        assert_eq!(
            contents[0].borrow().e_get("title"),
            Some(Val::String("The Library".into()))
        );
        let chapters = contents[0].borrow().e_get("chapters").unwrap();
        match &chapters {
            Val::List(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].as_object().unwrap().borrow().e_class(), "Chapter");
            }
            _ => panic!("expected containment list, got {chapters:?}"),
        }
    }

    #[test]
    fn load_bad_xmi_reports_error() {
        let registry = library_reg();
        let mut r = XMIResource::new(Uri::parse("file:///c.xmi"), registry);
        assert!(r.load_from_string("<lib:Unknown/>").is_err());
        assert!(!r.resource().is_loaded()); // failed load leaves not-loaded
    }
}
