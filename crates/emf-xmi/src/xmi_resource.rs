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

use emf_common::resource::{Resource, ResourceHandle};
use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use emf_ecore::PackageRegistry;

use super::loader::load_from_str;
use super::options::XmiOptions;
use super::saver::save_to_string;

/// An XMI-serializable [`Resource`] bound to a [`PackageRegistry`].
#[derive(Debug)]
pub struct XMIResource {
    resource: Resource,
    registry: PackageRegistry,
    opts: XmiOptions,
}

impl XMIResource {
    /// A new XMI resource at `uri`, resolving its metamodel via `registry`.
    pub fn new(uri: Uri, registry: PackageRegistry) -> Self {
        Self {
            resource: Resource::new(uri),
            registry,
            opts: XmiOptions::default(),
        }
    }

    /// Wrap an existing abstract [`Resource`].
    pub fn from_parts(resource: Resource, registry: PackageRegistry) -> Self {
        Self {
            resource,
            registry,
            opts: XmiOptions::default(),
        }
    }

    /// The serialization options (mutable for customization).
    pub fn options_mut(&mut self) -> &mut XmiOptions {
        &mut self.opts
    }

    /// Access the underlying abstract resource.
    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    /// Mutable access to the underlying abstract resource.
    pub fn resource_mut(&mut self) -> &mut Resource {
        &mut self.resource
    }

    /// Serialize the current contents to an XMI/XML string.
    pub fn save_to_string(&self) -> String {
        save_to_string(self.resource.contents(), &self.opts)
    }

    /// `toXmiString` — delegate to [`Self::save_to_string`].
    pub fn to_xmi_string(&self) -> String {
        self.save_to_string()
    }

    /// Parse XMI/XML text into the resource, replacing its root contents and
    /// marking it loaded. Any parse/registry error surfaces as `Err`.
    pub fn load_from_string(&mut self, src: &str) -> Result<(), String> {
        let roots = load_from_str(src, &self.registry)?;
        let contents: Vec<ObjectRef> = roots;
        self.resource.set_contents(contents);
        self.resource.set_loaded(true);
        Ok(())
    }

    /// `fromXmiString` — delegate to [`Self::load_from_string`].
    pub fn from_xmi_string(&mut self, src: &str) -> Result<(), String> {
        self.load_from_string(src)
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
