//! C++ parity suite: `E2E_GenModelXmiMultiEcoreTypedTests.cpp` — multi-ecore
//! typed cross-package references.
//!
//! Loads a `base` package (Library: name + books; Book: title) and an `ext`
//! package whose `AnnotatedLibrary` inherits `base#//Library` and adds `note`
//! (attribute) + `highlighted` (containment of `base#//Book`). Verifies the
//! typed contract Java EMF exposes for cross-package inheritance:
//!
//!   - AnnotatedLibrary inherits Library (cross-package eSuperTypes).
//!   - own `eStructuralFeatures` == {note, highlighted}.
//!   - `eAllStructuralFeatures` == inherited {name, books} + own {note,
//!     highlighted} (>= 4, all findable by name).
//!   - instantiate an AnnotatedLibrary; `eClass()` == "AnnotatedLibrary".
//!   - set inherited `name` and own `note` reflectively, read them back.
//!   - add Book children to inherited `books` and own `highlighted` lists.
//!   - cross-package containment `highlighted.eType` resolves to base#//Book.
//!   - `eAllContainments` includes inherited `books` + own `highlighted`.
//!   - save an AnnotatedLibrary instance: `ext:AnnotatedLibrary`, `name`,
//!     `note` present.
//!   - both packages' factories independently instantiate their classes.
//!
//! Rust mapping: `DynamicEObject` resolves the inheritance graph against the
//! shared [`PackageRegistry`]; C++'s per-package `EFactoryInstance` maps to
//! instantiating via the class resolved from the shared registry.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{make_package_ref, DynamicEObject, PackageRegistry};
use emf_ecore_codegen::loader::load_ecore_package;
use emf_xmi::XMIResource;

const K_BASE_ECORE: &str =
    r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="base" nsURI="http://example.com/e2e/typed/base" nsPrefix="base">
  <eClassifiers xsi:type="ecore:EClass" name="Library">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="books" upperBound="-1"
        eType="#//Book" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Book">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="title"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>
"##;

const K_EXT_ECORE: &str =
    r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="ext" nsURI="http://example.com/e2e/typed/ext" nsPrefix="ext">
  <eClassifiers xsi:type="ecore:EClass" name="AnnotatedLibrary"
      eSuperTypes="http://example.com/e2e/typed/base#//Library">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="note"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="highlighted" upperBound="-1"
        eType="ecore:EClass http://example.com/e2e/typed/base#//Book" containment="true"/>
  </eClassifiers>
</ecore:EPackage>
"##;

/// Load + register `base` then `ext`; returns the shared registry plus an
/// owned `ext` package snapshot (to look up AnnotatedLibrary).
fn register() -> (PackageRegistry, emf_ecore::EPackage) {
    let base = load_ecore_package(K_BASE_ECORE).unwrap();
    let ext = load_ecore_package(K_EXT_ECORE).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(base));
    reg.register(make_package_ref(ext.clone()));
    (reg, ext)
}

/// Instantiate an object whose class `name` resolves from the shared registry
/// (the Rust equivalent of `EFactoryInstance.create(EClass)`).
fn dyn_of(reg: &PackageRegistry, class: &str) -> ObjectRef {
    let cls = reg.find_class(class).unwrap();
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
}

/// The AnnotatedLibrary class declaration (used for reflection on the
/// meta-model), resolved against the shared registry.
fn annotated_library(reg: &PackageRegistry) -> emf_ecore::EClass {
    reg.find_class("AnnotatedLibrary").expect("AnnotatedLibrary")
}

/// 1) AnnotatedLibrary inherits Library (cross-package eSuperTypes).
#[test]
fn annotated_library_inherits_library() {
    let (reg, ext) = register();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let sups = ann.e_super_types();
    assert_eq!(sups.len(), 1);
    assert_eq!(sups[0], "Library");
    assert!(
        reg.find_class("Library").is_some(),
        "cross-package supertype resolves to base's Library"
    );
}

/// 2) Own features of AnnotatedLibrary: exactly {note, highlighted}.
#[test]
fn own_features_accessible() {
    let (reg, ext) = register();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let own = ann.e_structural_features();
    assert_eq!(own.len(), 2, "own features are note + highlighted");
    let _ = annotated_library(&reg);
    assert!(
        ann.feature_by_name("note", &reg).is_some(),
        "own attribute note accessible"
    );
    assert!(
        ann.feature_by_name("highlighted", &reg).is_some(),
        "own reference highlighted accessible"
    );
}

/// 3) eAllStructuralFeatures includes inherited {name, books} + own {note,
///    highlighted} (>= 4).
#[test]
fn inherited_features_in_all_features() {
    let (reg, ext) = register();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let all = ann.e_all_structural_features(&reg);
    assert!(
        all.len() >= 4,
        "all features must include inherited + own, got {}",
        all.len()
    );
    for name in ["name", "books", "note", "highlighted"] {
        assert!(
            ann.feature_by_name(name, &reg).is_some(),
            "feature {name} findable across the hierarchy"
        );
    }
}

/// 4) Instantiate an AnnotatedLibrary -> eClass() == "AnnotatedLibrary".
#[test]
fn instantiate_annotated_library() {
    let (reg, _) = register();
    let obj = dyn_of(&reg, "AnnotatedLibrary");
    assert_eq!(obj.borrow().e_class(), "AnnotatedLibrary");
}

/// 5) Set inherited attribute `name` reflectively.
#[test]
fn set_inherited_attribute() {
    let (reg, _) = register();
    let obj = dyn_of(&reg, "AnnotatedLibrary");
    obj.borrow_mut()
        .e_set("name", Val::String("Typed Library".into()));
    assert_eq!(
        obj.borrow().e_get("name"),
        Some(Val::String("Typed Library".into()))
    );
}

/// 6) Set own attribute `note`.
#[test]
fn set_own_attribute() {
    let (reg, _) = register();
    let obj = dyn_of(&reg, "AnnotatedLibrary");
    obj.borrow_mut()
        .e_set("note", Val::String("A note".into()));
    assert_eq!(
        obj.borrow().e_get("note"),
        Some(Val::String("A note".into()))
    );
}

/// 7) Add a Book to the inherited `books` containment list (base's Book).
#[test]
fn add_to_inherited_containment() {
    let (reg, _) = register();
    let ann = dyn_of(&reg, "AnnotatedLibrary");
    let book = dyn_of(&reg, "Book");
    book.borrow_mut()
        .e_set("title", Val::String("Inherited Book".into()));
    ann.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(book.clone())]));
    let list = ann_list(&ann, "books");
    assert_eq!(list.len(), 1);
    assert_eq!(Rc::as_ptr(&list[0]), Rc::as_ptr(&book));
}

/// 8) Add a Book to the own `highlighted` containment list.
#[test]
fn add_to_own_containment() {
    let (reg, _) = register();
    let ann = dyn_of(&reg, "AnnotatedLibrary");
    let book = dyn_of(&reg, "Book");
    ann.borrow_mut()
        .e_set("highlighted", Val::List(vec![Val::Object(book)]));
    assert_eq!(ann_list(&ann, "highlighted").len(), 1);
}

/// 9) Cross-package containment eType: highlighted -> base#//Book.
#[test]
fn cross_package_containment_e_type() {
    let (reg, ext) = register();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let feat = ann
        .feature_by_name("highlighted", &reg)
        .expect("highlighted feature");
    assert!(feat.is_reference(), "highlighted is a reference");
    assert!(feat.is_containment(), "highlighted is a containment reference");
    assert!(feat.is_many(), "highlighted is upperBound=-1");
    assert_eq!(feat.type_name().unwrap(), "Book");
    let book = reg.find_class("Book").expect("Book resolves");
    assert_eq!(book.name(), "Book");
}

/// 10) eAllContainments includes inherited `books` + own `highlighted`.
#[test]
fn all_containments_include_inherited() {
    let (reg, ext) = register();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let containments: Vec<_> = ann
        .e_all_references(&reg)
        .into_iter()
        .filter(|f| f.is_containment())
        .map(|f| f.name().to_string())
        .collect();
    assert!(
        containments.len() >= 2,
        "containments must include inherited + own: {containments:?}"
    );
    assert!(containments.contains(&"books".to_string()));
    assert!(containments.contains(&"highlighted".to_string()));
}

/// 11) Save an AnnotatedLibrary instance to XMI.
#[test]
fn save_annotated_library_instance() {
    let (reg, _) = register();
    let ann = dyn_of(&reg, "AnnotatedLibrary");
    ann.borrow_mut()
        .e_set("name", Val::String("Saved AnnLib".into()));
    ann.borrow_mut()
        .e_set("note", Val::String("Note text".into()));

    let mut r = XMIResource::new(Uri::parse("file:///out.xmi"), reg.clone());
    r.resource_mut().add_to_contents(ann);
    let out = r.save_to_string();

    assert!(out.contains("ext:AnnotatedLibrary"), "{out}");
    assert!(out.contains("name=\"Saved AnnLib\""), "{out}");
    assert!(out.contains("note=\"Note text\""), "{out}");
}

/// 12) Both packages instantiate their classes independently.
#[test]
fn factories_are_independent() {
    let (reg, _) = register();
    let book = dyn_of(&reg, "Book");
    let ann = dyn_of(&reg, "AnnotatedLibrary");
    // base factory creates a Book; ext factory creates an AnnotatedLibrary.
    assert_eq!(book.borrow().e_class(), "Book");
    assert_eq!(ann.borrow().e_class(), "AnnotatedLibrary");
    // Distinct instances, each typed by its own class.
    assert_ne!(Rc::as_ptr(&book), Rc::as_ptr(&ann));
}

fn ann_list(obj: &ObjectRef, feat: &str) -> Vec<ObjectRef> {
    match obj.borrow().e_get(feat) {
        Some(Val::List(items)) => items
            .iter()
            .filter_map(|v| v.as_object().map(|o| o.clone()))
            .collect(),
        _ => Vec::new(),
    }
}