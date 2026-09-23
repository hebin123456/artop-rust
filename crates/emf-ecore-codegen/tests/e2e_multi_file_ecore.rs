//! C++ parity suite: `E2E_MultiFileEcoreTests.cpp` contract.
//!
//! Multi-file `.ecore` loading and cross-file reference resolution: load a
//! `base` package (Library/Book/Writer), then an `ext` package whose
//! `AnnotatedLibrary` subclasses `base#//Library` and whose references point
//! at `base#//Book`. Ported with the Rust metadata loader
//! (`load_ecore_package`) + a shared [`PackageRegistry`]. Cross-file
//! references (`eSuperTypes` / EReference `eType`) resolve to the class that
//! `base` contributes to the combined registry, mirroring EMF `XMLHandler`
//! cross-resource resolution.
//!
//! Cases 7-10 read `samples/multi/*.ecore` on disk and are skip-gated (the C++
//! suite likewise skips when the samples are absent); cases 1-6 use inline
//! documents.

use emf_ecore::PackageRegistry;

const K_BASE_ECORE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<ecore:EPackage xmi:version=\"2.0\"\n",
    "    xmlns:xmi=\"http://www.omg.org/XMI\"\n",
    "    xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n",
    "    xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\"\n",
    "    name=\"base\" nsURI=\"http://example.com/e2e/multi/base\" nsPrefix=\"base\">\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"Library\">\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"name\"\n",
    "        eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"books\" upperBound=\"-1\"\n",
    "        eType=\"#//Book\" containment=\"true\"/>\n",
    "  </eClassifiers>\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"Book\">\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"title\"\n",
    "        eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>\n",
    "  </eClassifiers>\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"Writer\">\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"name\"\n",
    "        eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>\n",
    "  </eClassifiers>\n",
    "</ecore:EPackage>\n"
);

const K_EXT_ECORE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<ecore:EPackage xmi:version=\"2.0\"\n",
    "    xmlns:xmi=\"http://www.omg.org/XMI\"\n",
    "    xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n",
    "    xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\"\n",
    "    name=\"ext\" nsURI=\"http://example.com/e2e/multi/ext\" nsPrefix=\"ext\">\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"AnnotatedLibrary\"\n",
    "      eSuperTypes=\"http://example.com/e2e/multi/base#//Library\">\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"note\"\n",
    "        eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"highlighted\" upperBound=\"-1\"\n",
    "        eType=\"ecore:EClass http://example.com/e2e/multi/base#//Book\" containment=\"true\"/>\n",
    "  </eClassifiers>\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"BookCollection\">\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"label\"\n",
    "        eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>\n",
    "    <eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"books\" upperBound=\"-1\"\n",
    "        eType=\"ecore:EClass http://example.com/e2e/multi/base#//Book\" containment=\"true\"/>\n",
    "  </eClassifiers>\n",
    "</ecore:EPackage>\n"
);

const BASE_NS: &str = "http://example.com/e2e/multi/base";

/// Register `base` then `ext` into a fresh shared registry; returns the shared
/// registry plus owned snapshots of the two packages.
fn register_base_and_ext() -> (PackageRegistry, emf_ecore::EPackage, emf_ecore::EPackage) {
    let base = emf_ecore_codegen::loader::load_ecore_package(K_BASE_ECORE).unwrap();
    let ext = emf_ecore_codegen::loader::load_ecore_package(K_EXT_ECORE).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(base.clone())));
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(ext.clone())));
    (reg, base, ext)
}

/// 1) Base package loaded and registered by nsURI.
#[test]
fn base_package_loaded_and_registered() {
    let (reg, base, _) = register_base_and_ext();
    assert_eq!(base.name(), "base");
    assert_eq!(base.ns_uri().unwrap().to_string(), BASE_NS);
    assert!(
        reg.contains_key(BASE_NS),
        "registry must resolve base by its nsURI"
    );
}

/// 2) Ext package loaded after base; structure verified.
#[test]
fn ext_package_loaded_after_base() {
    let (_, _, ext) = register_base_and_ext();
    assert_eq!(ext.name(), "ext");
    assert_eq!(ext.classes().len(), 2);
    assert!(ext.find_class("AnnotatedLibrary").is_some());
    assert!(ext.find_class("BookCollection").is_some());
}

/// 3) Cross-package eSuperTypes: AnnotatedLibrary inherits base#//Library.
#[test]
fn cross_package_super_types_resolved() {
    let (reg, base, ext) = register_base_and_ext();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let sups = ann.e_super_types();
    assert_eq!(sups.len(), 1);
    assert_eq!(sups[0], "Library");
    // Resolves to base's Library class (non-proxy).
    let base_lib = reg.find_class("Library").expect("Library resolves");
    assert_eq!(base_lib.name(), "Library");
    assert!(base.find_class("Library").is_some());
}

/// 4) Cross-package eType: AnnotatedLibrary.highlighted -> base#//Book.
#[test]
fn cross_package_e_type_highlighted_resolves() {
    let (reg, _, ext) = register_base_and_ext();
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let feat = ann
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "highlighted")
        .expect("highlighted feature");
    assert!(feat.is_reference(), "highlighted must be a reference");
    assert_eq!(feat.type_name().unwrap(), "Book");
    assert!(reg.find_class("Book").is_some(), "Book resolves");
}

/// 5) Cross-package eType: BookCollection.books -> base#//Book.
#[test]
fn book_collection_books_e_type_resolves() {
    let (reg, _, ext) = register_base_and_ext();
    let bc = ext.find_class("BookCollection").expect("BookCollection");
    let feat = bc
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "books")
        .expect("books feature");
    assert!(feat.is_reference(), "books must be a reference");
    assert!(feat.is_many(), "books is upperBound=-1");
    assert!(feat.is_containment(), "books is a containment reference");
    assert_eq!(feat.type_name().unwrap(), "Book");
    assert!(reg.find_class("Book").is_some(), "Book resolves");
}

/// 6) Both packages present in the registry simultaneously.
#[test]
fn both_packages_in_registry() {
    let (reg, _, _) = register_base_and_ext();
    assert!(reg.contains_key(BASE_NS));
    assert!(reg.contains_key("http://example.com/e2e/multi/ext"));
}

/// Helper for the sample-file cases 7-10 (skip-gated like the C++ suite; the
/// files live under the matching `samples/multi` directory in this crate).
fn sample(name: &str) -> Option<String> {
    let path = {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/samples/multi");
        p.push(name);
        p
    };
    std::fs::read_to_string(&path).ok()
}

/// 7) Load samples/multi/library.ecore from disk.
#[test]
fn load_sample_library_ecore() {
    let src = match sample("library.ecore") {
        Some(s) => s,
        None => return, // miss; C++ suite skips too
    };
    let pkg = emf_ecore_codegen::loader::load_ecore_package(&src).unwrap();
    assert_eq!(pkg.name(), "library");
    assert_eq!(pkg.ns_uri().unwrap().to_string(), "http://example.com/e2e/library");
    assert_eq!(pkg.classes().len(), 3);
    assert!(pkg.find_class("Library").is_some());
    assert!(pkg.find_class("Book").is_some());
    assert!(pkg.find_class("Writer").is_some());
}

/// 8) Load samples/multi/library_ext.ecore from disk.
#[test]
fn load_sample_library_ext_ecore() {
    let base_src = match sample("library.ecore") {
        Some(s) => s,
        None => return,
    };
    let ext_src = match sample("library_ext.ecore") {
        Some(s) => s,
        None => return,
    };
    let base = emf_ecore_codegen::loader::load_ecore_package(&base_src).unwrap();
    let ext = emf_ecore_codegen::loader::load_ecore_package(&ext_src).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(base)));
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(ext.clone())));
    assert_eq!(ext.name(), "libraryExt");
    assert_eq!(ext.classes().len(), 2);
    assert!(ext.find_class("AnnotatedLibrary").is_some());
    assert!(ext.find_class("BookCollection").is_some());
}

/// 9) Sample cross-file eSuperTypes: AnnotatedLibrary inherits Library.
#[test]
fn sample_cross_file_super_types_resolved() {
    let base_src = match sample("library.ecore") {
        Some(s) => s,
        None => return,
    };
    let ext_src = match sample("library_ext.ecore") {
        Some(s) => s,
        None => return,
    };
    let base = emf_ecore_codegen::loader::load_ecore_package(&base_src).unwrap();
    let ext = emf_ecore_codegen::loader::load_ecore_package(&ext_src).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(base)));
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(ext.clone())));
    let ann = ext.find_class("AnnotatedLibrary").expect("AnnotatedLibrary");
    let sups = ann.e_super_types();
    assert_eq!(sups.len(), 1);
    assert_eq!(reg.find_class(&sups[0]).map(|c| c.name().to_string()), Some("Library".to_string()));
}

/// 10) Sample cross-file eType: BookCollection.books -> Book.
#[test]
fn sample_cross_file_e_type_resolved() {
    let base_src = match sample("library.ecore") {
        Some(s) => s,
        None => return,
    };
    let ext_src = match sample("library_ext.ecore") {
        Some(s) => s,
        None => return,
    };
    let base = emf_ecore_codegen::loader::load_ecore_package(&base_src).unwrap();
    let ext = emf_ecore_codegen::loader::load_ecore_package(&ext_src).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(base)));
    reg.register(std::rc::Rc::new(std::cell::RefCell::new(ext.clone())));
    let bc = ext.find_class("BookCollection").expect("BookCollection");
    let books = bc
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "books")
        .expect("books feature");
    assert_eq!(books.type_name().unwrap(), "Book");
    assert!(reg.find_class("Book").is_some(), "Book resolves");
}