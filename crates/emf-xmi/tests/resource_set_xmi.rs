//! End-to-end: write a real `.xmi` file through a `ResourceSet` + factory,
//! then load it back on demand from another set and validate the reconstructed
//! object model. Exercises the full static-modeling path: XML file -> element
//! tree -> `DynamicEObject` graph (class name + attributes + containment).

use emf_common::eobject::EObject;
use emf_common::resource::ResourceSet;
use emf_common::uri::Uri;
use emf_common::value::Val;
use emf_ecore::{
    make_package_ref, DynamicEObject, EClass, EClassKind, EStructuralFeature, PackageRegistry,
};
use std::cell::RefCell;
use std::rc::Rc;

use emf_xmi::{XMIResource, XMIResourceFactory};

const NS: &str = "http://example.org/library";

fn library_registry() -> PackageRegistry {
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

fn new_book(registry: &PackageRegistry) -> emf_common::value::ObjectRef {
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
fn resource_set_writes_and_reloads_xmi_file() {
    let registry = library_registry();

    // Unique temp file path owned by this process.
    let path = std::env::temp_dir().join(format!("rs_xmi_e2e_{}.xmi", std::process::id()));
    let uri = Uri::create_file_uri(&path.to_string_lossy());

    // Set A: register a factory, create the resource, populate and save to disk.
    {
        let mut set = ResourceSet::new();
        set.set_resource_factory(Box::new(XMIResourceFactory::new(registry.clone())));

        let xres: &mut XMIResource = set
            .create_resource(uri.clone())
            .as_any_mut()
            .downcast_mut::<XMIResource>()
            .expect("factory produced an XMIResource");

        xres.resource_mut().set_root(new_book(&registry));
        xres.resource_mut().set_modified(true);
        xres.save().expect("save must succeed");
    }

    // Set B: no prepopulation — getResource(uri, true) creates + loads on demand.
    {
        let mut set = ResourceSet::new();
        set.set_resource_factory(Box::new(XMIResourceFactory::new(registry.clone())));

        let loaded = set
            .get_resource(&uri, true)
            .expect("on-demand load must not error")
            .expect("resource created on demand");
        assert!(loaded.is_loaded());

        let xres: &XMIResource = loaded.as_any().downcast_ref::<XMIResource>().unwrap();
        let contents = xres.resource().contents();
        assert_eq!(contents.len(), 1);

        let root = &contents[0];
        assert_eq!(root.borrow().e_class(), "Book");
        assert_eq!(
            root.borrow().e_get("title"),
            Some(Val::String("The Library".into()))
        );

        let chapters = root.borrow().e_get("chapters").expect("chapters set");
        match &chapters {
            Val::List(items) => {
                assert_eq!(items.len(), 1);
                let chap = items[0].as_object().unwrap();
                assert_eq!(chap.borrow().e_class(), "Chapter");
                assert_eq!(
                    chap.borrow().e_get("name"),
                    Some(Val::String("Chapter 1".into()))
                );
            }
            other => panic!("expected containment list, got {other:?}"),
        }
    }

    let _ = std::fs::remove_file(&path);
}
