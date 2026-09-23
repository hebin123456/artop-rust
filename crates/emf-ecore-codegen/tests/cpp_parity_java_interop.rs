//! C++ parity suite: `JavaInteropTests.cpp` — interoperability with a
//! Java-EMF-generated `.ecore` document (read-in and save-back).
//!
//! The C++ test reads two reference files produced by Java EMF
//! (`java_ref/library.ecore` + `.xmi`). Those files are absent in this repo,
//! so per the C++ `skip` contract we model the *same* shape: a standalone
//! `<ecore:EPackage>` document (root element, no external package registration
//! required) carrying ≥4 classifiers that include classes, an enum and a data
//! type. The Rust back-end (XMI loader + metamodel saver) must:
//!   1. load it by URI as an independent EPackage: name / nsURI / nsPrefix and
//!      ≥4 classifiers;
//!   2. save it back: exactly one `<ecore:EPackage>` wrapper, preserving
//!      nsURI / name / nsPrefix and re-emitting classes, the enum and the
//!      data type with their `xsi:type` markers.
//!
//! Aligned to Java `org.eclipse.emf.test.tools/data/ant.expected` documents.

use emf_xmi::metamodel_saver::save_ecore_package;
use emf_xmi::options::XmiOptions;

const K_JAVA_LIKE_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="library"
    nsURI="http://example.com/emfdemo/library"
    nsPrefix="library">
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
  <eClassifiers xsi:type="ecore:EEnum" name="BookCategory">
    <eLiterals name="FICTION"/>
    <eLiterals name="SCIENCE" value="1"/>
    <eLiterals name="TECHNICAL" value="2"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EDataType" name="MyString"
      instanceClassName="org.example.MyString"/>
</ecore:EPackage>"##;

/// Load the standalone Java-like `.ecore` as an independent EPackage document
/// (C++ `JavaInterop_LoadJavaLibraryEcore`).
#[test]
fn load_java_like_library_ecore() {
    let pkg = emf_ecore_codegen::loader::load_ecore_package(K_JAVA_LIKE_ECORE).unwrap();
    assert_eq!(pkg.name(), "library");
    assert_eq!(
        pkg.ns_uri().unwrap().to_string(),
        "http://example.com/emfdemo/library"
    );
    assert_eq!(pkg.ns_prefix(), "library");

    // C++ asserts >= 4 classifiers: Library, Book (classes) + BookCategory
    // (enum) + MyString (data type).
    let total = pkg.classes().len() + pkg.enums().len() + pkg.data_types().len();
    assert!(
        total >= 4,
        "expected at least 4 classifiers, found {total}"
    );

    // Library and Book resolve as classes.
    let lib = pkg.find_class("Library").expect("Library class");
    assert_eq!(lib.name(), "Library");
    let book = pkg.find_class("Book").expect("Book class");
    assert_eq!(book.name(), "Book");

    // BookCategory is an enum with literals.
    let cat = pkg.find_enum("BookCategory").expect("BookCategory enum");
    assert!(cat.e_literals().len() >= 3, "enum has literals");

    // A data type survives.
    assert!(
        pkg.data_types().iter().any(|d| d.name() == "MyString"),
        "MyString data type present"
    );
}

/// Save the same document back: single `<ecore:EPackage>` wrapper preserving
/// nsURI/name/nsPrefix and re-emitting classes, the enum and the data type with
/// their `xsi:type` markers (C++ `JavaInterop_SaveBackJavaLibraryEcore`).
#[test]
fn save_back_java_like_library_ecore() {
    let pkg = emf_ecore_codegen::loader::load_ecore_package(K_JAVA_LIKE_ECORE).unwrap();
    let out = save_ecore_package(&pkg, &XmiOptions::default());

    // Metadata preserved.
    assert!(
        out.contains("nsURI=\"http://example.com/emfdemo/library\""),
        "{out}"
    );
    assert!(out.contains("name=\"library\""), "{out}");
    assert!(out.contains("nsPrefix=\"library\""), "{out}");

    // Classifiers.
    assert!(out.contains("BookCategory"), "{out}");
    assert!(out.contains("Library"), "{out}");
    assert!(
        out.contains("xsi:type=\"ecore:EClass\""),
        "classes carry xsi:type, {out}"
    );
    assert!(
        out.contains("xsi:type=\"ecore:EEnum\""),
        "enum carries xsi:type, {out}"
    );
    assert!(
        out.contains("xsi:type=\"ecore:EDataType\""),
        "data type carries xsi:type, {out}"
    );

    // Exactly one <ecore:EPackage> wrapper.
    let first = out.find("<ecore:EPackage");
    assert!(first.is_some(), "must open an EPackage");
    if let Some(p) = first {
        let second = out[p + 1..].find("<ecore:EPackage");
        assert!(
            second.is_none(),
            "exactly one <ecore:EPackage> wrapper expected"
        );
    }
}

/// The loaded `Library` carries its class feature surface (name attribute +
/// containment `books` reference to Book), mirroring the loader contract.
#[test]
fn loaded_library_exposes_features() {
    let pkg = emf_ecore_codegen::loader::load_ecore_package(K_JAVA_LIKE_ECORE).unwrap();
    let lib = pkg.find_class("Library").unwrap();
    let feats = lib.e_structural_features();
    assert!(!feats.is_empty());
    let name = feats.iter().find(|f| f.name() == "name").unwrap();
    assert!(!name.is_reference());
    let books = feats.iter().find(|f| f.name() == "books").unwrap();
    assert!(books.is_reference() && books.is_containment() && books.is_many());
    assert_eq!(books.type_name().unwrap(), "Book");
}