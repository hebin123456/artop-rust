//! C++ parity suite: `E2E_GenModelXmiTests.cpp` — end-to-end gen-model XMI
//! pipeline.
//!
//! Ports the 10 end-to-end assertions that chain the whole flow in one place:
//! load an `.ecore` metamodel -> instantiate dynamic model objects -> reflect
//! attributes / containment -> `saveToString` as an XMI instance document ->
//! reload -> verify structural equivalence. Aligned to Java
//! `XMIResourceImpl`'s load/save path.
//!
//!   1. Load `.ecore`: EPackage name/nsURI/nsPrefix + 3 classifiers.
//!   2. Instantiate a Library and set `name` via reflection.
//!   3. Save yields an XMI instance with `library:Library` + book fields.
//!   4. Save -> reload preserves the Library `name`.
//!   5. Save -> reload preserves containment children (2 books).
//!   6. Save -> reload preserves Book attributes, `pages` as an EInt value.
//!   7. Empty Library save -> reload keeps an empty `books` list.
//!   8. The package becomes resolvable from the registry (what makes the
//!      reloaded instance document parse).
//!   9. Multiple roots are wrapped in `<xmi:XMI>`.
//!  10. save -> reload -> save is idempotent (two saves byte-identical).

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{make_package_ref, DynamicEObject, PackageRegistry};
use emf_ecore_codegen::loader::load_ecore_package;
use emf_xmi::XMIResource;

const K_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="library" nsURI="http://example.com/e2e/genmodel/library" nsPrefix="library">
  <eClassifiers xsi:type="ecore:EClass" name="Library">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="books" upperBound="-1"
        eType="#//Book" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Book">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="title"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="pages"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Writer">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>
"##;

const NS_URI: &str = "http://example.com/e2e/genmodel/library";

/// Load and register the Ecore package, the one setup step that lets instance
/// documents resolve their classes (C++ `EPackageRegistry` auto-registration
/// after loading the metamodel).
fn registry() -> PackageRegistry {
    let pkg = load_ecore_package(K_ECORE).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

fn dyn_of(reg: &PackageRegistry, class: &str) -> ObjectRef {
    let cls = reg.find_class(class).unwrap();
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
}

/// Save a root (or roots) to an XMI string through a resource.
fn save(reg: &PackageRegistry, roots: &[ObjectRef]) -> String {
    let mut r = XMIResource::new(Uri::parse("file:///out.xmi"), reg.clone());
    for root in roots {
        r.resource_mut().add_to_contents(root.clone());
    }
    r.save_to_string()
}

/// Reload an XMI string into a fresh resource against `reg`, returning roots.
fn reload(reg: &PackageRegistry, xmi: &str) -> Vec<ObjectRef> {
    let mut r = XMIResource::new(Uri::parse("file:///in.xmi"), reg.clone());
    r.load_from_string(xmi).unwrap();
    r.resource().contents().to_vec()
}

fn list_of(obj: &ObjectRef, feat: &str) -> Vec<ObjectRef> {
    match obj.borrow().e_get(feat) {
        Some(Val::List(items)) => items
            .iter()
            .filter_map(|v| v.as_object().map(|o| o.clone()))
            .collect(),
        _ => Vec::new(),
    }
}

/// 1) Load `.ecore`: EPackage metadata + exactly 3 classifiers.
#[test]
fn load_ecore_verify_structure() {
    let pkg = load_ecore_package(K_ECORE).unwrap();
    assert_eq!(pkg.name(), "library");
    assert_eq!(pkg.ns_uri().unwrap().to_string(), NS_URI);
    assert_eq!(pkg.ns_prefix(), "library");
    assert_eq!(pkg.classes().len(), 3);
    assert!(pkg.find_class("Library").is_some());
    assert!(pkg.find_class("Book").is_some());
    assert!(pkg.find_class("Writer").is_some());
}

/// 2) Instantiate a Library via the registry and set `name` reflectively.
#[test]
fn instantiate_library_set_name() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("City Library".into()));
    assert_eq!(
        lib.borrow().e_get("name"),
        Some(Val::String("City Library".into()))
    );
}

/// 3) Save produces an XMI instance document with the expected fields.
#[test]
fn instantiate_and_save_produces_xmi() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("Central Library".into()));

    let b0 = dyn_of(&reg, "Book");
    b0.borrow_mut()
        .e_set("title", Val::String("Book A".into()));
    b0.borrow_mut().e_set("pages", Val::Int(100));

    let b1 = dyn_of(&reg, "Book");
    b1.borrow_mut()
        .e_set("title", Val::String("Book B".into()));
    b1.borrow_mut().e_set("pages", Val::Int(200));

    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(b0), Val::Object(b1)]));

    let out = save(&reg, &[lib]);
    assert!(out.contains("library:Library"), "{out}");
    assert!(out.contains("name=\"Central Library\""), "{out}");
    assert!(out.contains("title=\"Book A\""), "{out}");
    assert!(out.contains("title=\"Book B\""), "{out}");
    assert!(out.contains("pages=\"100\""), "{out}");
    assert!(out.contains("pages=\"200\""), "{out}");
}

/// 4) Save -> reload preserves the Library `name`.
#[test]
fn save_and_reload_library_name_preserved() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("Reload Test Library".into()));

    let xmi = save(&reg, &[lib]);
    let loaded = reload(&reg, &xmi);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].borrow().e_class(), "Library");
    assert_eq!(
        loaded[0].borrow().e_get("name"),
        Some(Val::String("Reload Test Library".into()))
    );
}

/// 5) Save -> reload preserves containment children (2 books).
#[test]
fn save_and_reload_containment_children_preserved() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("Lib With Books".into()));
    let b0 = dyn_of(&reg, "Book");
    b0.borrow_mut().e_set("title", Val::String("T1".into()));
    let b1 = dyn_of(&reg, "Book");
    b1.borrow_mut().e_set("title", Val::String("T2".into()));
    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(b0), Val::Object(b1)]));

    let xmi = save(&reg, &[lib]);
    let loaded = reload(&reg, &xmi);
    let books = list_of(&loaded[0], "books");
    assert_eq!(books.len(), 2, "both books survive the roundtrip");
}

/// 6) Save -> reload preserves Book attributes, `pages` as an int value.
#[test]
fn save_and_reload_book_attributes_preserved() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut().e_set("name", Val::String("L".into()));
    let book = dyn_of(&reg, "Book");
    book.borrow_mut()
        .e_set("title", Val::String("Deep Book".into()));
    book.borrow_mut().e_set("pages", Val::Int(42));
    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(book)]));

    let xmi = save(&reg, &[lib]);
    let loaded = reload(&reg, &xmi);
    let books = list_of(&loaded[0], "books");
    assert_eq!(books.len(), 1);
    let b = &books[0];
    assert_eq!(b.borrow().e_class(), "Book");
    assert_eq!(
        b.borrow().e_get("title"),
        Some(Val::String("Deep Book".into()))
    );
    assert_eq!(
        b.borrow().e_get("pages"),
        Some(Val::Int(42)),
        "EInt attribute roundtrips as an integer value"
    );
}

/// 7) Empty Library save -> reload keeps an empty `books` list.
#[test]
fn empty_library_save_and_reload() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut().e_set("name", Val::String("Empty".into()));

    let xmi = save(&reg, &[lib]);
    let loaded = reload(&reg, &xmi);
    assert_eq!(loaded[0].borrow().e_class(), "Library");
    let books = list_of(&loaded[0], "books");
    assert!(books.is_empty(), "no children for an empty library");
}

/// 8) The loaded package resolves from the registry (the reload relies on it).
#[test]
fn package_resolvable_from_registry() {
    let reg = registry();
    // find_class resolves through the registered package by classifier name,
    // which is exactly what the instance loader uses to type a `library:Library`
    // root back to the `Library` EClass.
    assert_eq!(reg.find_class("Book").unwrap().name(), "Book");
    assert!(reg.find_package_of_class("Library").is_some());
}

/// 9) Multiple roots are wrapped in `<xmi:XMI>`.
#[test]
fn multi_root_save_wrapped_in_xmi_xmi() {
    let reg = registry();
    let w0 = dyn_of(&reg, "Writer");
    w0.borrow_mut().e_set("name", Val::String("Author A".into()));
    let w1 = dyn_of(&reg, "Writer");
    w1.borrow_mut().e_set("name", Val::String("Author B".into()));

    let out = save(&reg, &[w0, w1]);
    assert!(out.contains("<xmi:XMI"), "{out}");
    assert!(out.contains("</xmi:XMI>"), "{out}");
    assert!(out.contains("name=\"Author A\""), "{out}");
    assert!(out.contains("name=\"Author B\""), "{out}");
}

/// 10) save -> reload -> save is idempotent (two saves byte-identical).
#[test]
fn save_reload_save_idempotent() {
    let reg = registry();
    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("Idempotent Lib".into()));
    let book = dyn_of(&reg, "Book");
    book.borrow_mut().e_set("title", Val::String("IB".into()));
    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(book)]));

    let xmi1 = save(&reg, &[lib]);
    let loaded = reload(&reg, &xmi1);
    let xmi2 = save(&reg, &loaded);
    assert_eq!(xmi1, xmi2, "re-saving the reloaded resource is identical");
}