//! C++ parity suite: `E2E_GenModelXmiTypedMultiFileTests.cpp` — multi-file
//! typed XMI instance loading.
//!
//! Loads a set of Java-EMF-produced instance documents (`library.xmi`,
//! `authors.xmi`, `publishers.xmi` under `samples/multi-xmi-java`) that share
//! the single metamodel nsURI `http://example.com/emfdemo/library`. The
//! metamodel is loaded inline first (registered by nsURI), then each instance
//! document is parsed against it:
//!
//!   - library.xmi: single `<library:Library>` root; name attribute; books
//!     (2) / magazines / authors (3) / publishers (2) containment children.
//!   - authors.xmi: `<xmi:XMI>`-wrapped multi-root, 3 `<library:Author>`.
//!   - publishers.xmi: `<xmi:XMI>`-wrapped multi-root, 2 `<library:Publisher>`,
//!     each with a nested `address` containment (city "Sebastopol").
//!   - multiple files load into independent resources without interference;
//!     every loaded instance's class resolves through the same registered
//!     metamodel package.
//!
//! The Java files carry attributes the metamodel does not declare (e.g. Book
//! `publishDate`/`category`/`isbn`/`price`/`publisher`/`authors`), which must
//! be skipped on load (EMF record-and-skip semantics).

use std::path::PathBuf;

use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use emf_ecore::{make_package_ref, PackageRegistry};
use emf_ecore_codegen::loader::load_ecore_package;
use emf_xmi::XMIResource;

const NS_URI: &str = "http://example.com/emfdemo/library";

const K_ECORE: &str =
    r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="library" nsURI="http://example.com/emfdemo/library" nsPrefix="library">
  <eClassifiers xsi:type="ecore:EClass" name="Library">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="books" upperBound="-1"
        eType="#//Book" containment="true"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="magazines" upperBound="-1"
        eType="#//Magazine" containment="true"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="authors" upperBound="-1"
        eType="#//Author" containment="true"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="publishers" upperBound="-1"
        eType="#//Publisher" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Book">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="title"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="pages"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Magazine">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="title"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="issueNumber"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Author">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="email"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="birthYear"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Publisher">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="name"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="email"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EReference" name="address"
        eType="#//Address" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Address">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="street"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="city"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="country"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="zipCode"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>
"##;

/// Load the metamodel and register it; returns the registry keyed by nsURI.
fn registry() -> PackageRegistry {
    let pkg = load_ecore_package(K_ECORE).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

/// Read a Java-produced sample instance file into a fresh resource, returning
/// the resource's root contents.
fn load_sample(reg: &PackageRegistry, name: &str) -> Vec<ObjectRef> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/samples/multi-xmi-java");
    p.push(name);
    let src = std::fs::read_to_string(&p).expect("sample file present");
    let mut r = XMIResource::new(Uri::parse("file:///in.xmi"), reg.clone());
    r.load_from_string(&src).expect("load from string");
    r.resource().contents().to_vec()
}

fn str_of(obj: &ObjectRef, feat: &str) -> String {
    match obj.borrow().e_get(feat) {
        Some(emf_common::value::Val::String(s)) => s,
        _ => panic!("feature {feat} not a string"),
    }
}

fn list_of(obj: &ObjectRef, feat: &str) -> Vec<ObjectRef> {
    match obj.borrow().e_get(feat) {
        Some(emf_common::value::Val::List(items)) => items
            .iter()
            .filter_map(|v| v.as_object().map(|o| o.clone()))
            .collect(),
        // A single-valued containment is stored as a bare object.
        Some(emf_common::value::Val::Object(o)) => vec![o.clone()],
        _ => Vec::new(),
    }
}

/// 1) Metamodel loads and is registered to the registry by its nsURI.
#[test]
fn meta_model_registered_to_registry() {
    let pkg = load_ecore_package(K_ECORE).unwrap();
    assert_eq!(pkg.ns_uri().unwrap().to_string(), NS_URI);
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    assert!(reg.contains_key(NS_URI), "registry resolves by nsURI");
    // Classifiers present for all instance shapes the sample files use.
    for cls in ["Library", "Book", "Magazine", "Author", "Publisher", "Address"] {
        assert!(reg.find_class(cls).is_some(), "class {cls} registered");
    }
}

/// 2) library.xmi -> single Library root.
#[test]
fn load_library_xmi_single_root() {
    let reg = registry();
    let roots = load_sample(&reg, "library.xmi");
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].borrow().e_class(), "Library");
}

/// 3) library.xmi -> Library `name` loaded correctly.
#[test]
fn library_name_loaded_correctly() {
    let reg = registry();
    let roots = load_sample(&reg, "library.xmi");
    assert_eq!(str_of(&roots[0], "name"), "City Central Library");
}

/// 4) library.xmi -> books containment loaded (2), first title correct.
#[test]
fn library_books_containment_loaded() {
    let reg = registry();
    let roots = load_sample(&reg, "library.xmi");
    let books = list_of(&roots[0], "books");
    assert_eq!(books.len(), 2, "library.xmi has 2 books");
    assert_eq!(books[0].borrow().e_class(), "Book");
    assert_eq!(str_of(&books[0], "title"), "The Pragmatic Programmer");
}

/// 5) library.xmi -> authors containment loaded (3), first name correct.
#[test]
fn library_authors_containment_loaded() {
    let reg = registry();
    let roots = load_sample(&reg, "library.xmi");
    let authors = list_of(&roots[0], "authors");
    assert_eq!(authors.len(), 3, "library.xmi has 3 authors");
    assert_eq!(authors[0].borrow().e_class(), "Author");
    assert_eq!(str_of(&authors[0], "name"), "Ada Lovelace");
}

/// 6) library.xmi -> publishers containment loaded (2), first name correct.
#[test]
fn library_publishers_containment_loaded() {
    let reg = registry();
    let roots = load_sample(&reg, "library.xmi");
    let pubs = list_of(&roots[0], "publishers");
    assert_eq!(pubs.len(), 2, "library.xmi has 2 publishers");
    assert_eq!(pubs[0].borrow().e_class(), "Publisher");
    assert_eq!(str_of(&pubs[0], "name"), "O'Reilly Media");
}

/// 7) authors.xmi -> `<xmi:XMI>`-wrapped multi-root, 3 Author roots.
#[test]
fn load_authors_xmi_multi_root() {
    let reg = registry();
    let roots = load_sample(&reg, "authors.xmi");
    assert_eq!(roots.len(), 3);
    for r in &roots {
        assert_eq!(r.borrow().e_class(), "Author");
    }
}

/// 8) authors.xmi -> Author `name` / `email` attributes correct.
#[test]
fn authors_attributes_loaded_correctly() {
    let reg = registry();
    let roots = load_sample(&reg, "authors.xmi");
    assert_eq!(roots.len(), 3);
    assert_eq!(str_of(&roots[0], "name"), "Ada Lovelace");
    assert_eq!(str_of(&roots[0], "email"), "ada@example.com");
}

/// 9) publishers.xmi -> `<xmi:XMI>`-wrapped multi-root, 2 Publisher roots.
#[test]
fn load_publishers_xmi_multi_root() {
    let reg = registry();
    let roots = load_sample(&reg, "publishers.xmi");
    assert_eq!(roots.len(), 2);
    for r in &roots {
        assert_eq!(r.borrow().e_class(), "Publisher");
    }
}

/// 10) publishers.xmi -> Publisher `name` + nested `address` (city).
#[test]
fn publisher_address_nested_containment() {
    let reg = registry();
    let roots = load_sample(&reg, "publishers.xmi");
    assert_eq!(roots.len(), 2);
    assert_eq!(str_of(&roots[0], "name"), "O'Reilly Media");
    let addrs = list_of(&roots[0], "address");
    assert_eq!(addrs.len(), 1, "publisher has one nested address");
    assert_eq!(addrs[0].borrow().e_class(), "Address");
    assert_eq!(str_of(&addrs[0], "city"), "Sebastopol");
}

/// 11) Multiple files load into independent resources without interference.
#[test]
fn multiple_files_no_interference() {
    let reg = registry();
    let lib = load_sample(&reg, "library.xmi");
    assert_eq!(lib.len(), 1);
    assert_eq!(lib[0].borrow().e_class(), "Library");

    let auth = load_sample(&reg, "authors.xmi");
    assert_eq!(auth.len(), 3);
    for r in &auth {
        assert_eq!(r.borrow().e_class(), "Author");
    }
}

/// 12) Every loaded instance's class resolves through the same registered
///     metamodel package (nsURI `http://example.com/emfdemo/library`).
#[test]
fn all_files_use_same_registered_package() {
    let reg = registry();
    for name in ["library.xmi", "authors.xmi", "publishers.xmi"] {
        let roots = load_sample(&reg, name);
        for r in &roots {
            let class = r.borrow().e_class().to_string();
            let pkg = reg
                .find_package_of_class(&class)
                .unwrap_or_else(|| panic!("class {class} resolves to a package"));
            assert_eq!(
                pkg.borrow().ns_uri().unwrap().to_string(),
                NS_URI,
                "{name} instance {class} from the registered package"
            );
        }
    }
}