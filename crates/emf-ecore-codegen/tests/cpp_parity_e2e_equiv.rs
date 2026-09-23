//! C++ parity suite: `E2E_GenModelXmiEquivalentReplacementTests.cpp` —
//! C++/Java equivalent-replacement XMI contract.
//!
//! Ports the 10 assertions pinning C++ ↔ Java `.ecore` equivalence:
//!   1. A Java-style `.ecore` loads into the EPackage structure.
//!   2. Saving back emits Java-format elements (`<ecore:EPackage`,
//!      `xmlns:ecore/xmi/xsi`, `xsi:type="ecore:EClass/EAttribute/EReference"`).
//!   3. The save emits exactly one `<ecore:EPackage>` root (no extra wrapper).
//!   4. load -> save -> reload preserves package name/nsURI/nsPrefix.
//!   5. ... preserves classifier count and names.
//!   6. ... preserves an EAttribute's eType (EString).
//!   7. ... preserves an EReference's containment + target class.
//!   8. ... preserves defaultValueLiteral.
//!   9. Two saves are byte-identical (idempotent).
//!  10. The actual Java-produced sample file parses and saves back with all
//!      key metadata retained.
//!
//! Aligned to Java `XMIResourceImpl`'s `.ecore` read/write, ensuring C++ (and
//! Rust) output stays structurally equivalent to what Java EMF can read back.

use std::path::{Path, PathBuf};

use emf_ecore::EPackage;
use emf_ecore_codegen::loader::load_ecore_package;
use emf_xmi::metamodel_saver::save_ecore_package;
use emf_xmi::options::XmiOptions;

/// The C++ `kJavaStyleEcore` (inline copy so the suite never skips on a
/// missing file).
const K_JAVA_STYLE_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="library" nsURI="http://example.com/e2e/equiv/library" nsPrefix="library">
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
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt" defaultValueLiteral="0"/>
  </eClassifiers>
</ecore:EPackage>
"##;

fn load(xml: &str) -> EPackage {
    load_ecore_package(xml).unwrap()
}

fn save(pkg: &EPackage) -> String {
    save_ecore_package(pkg, &XmiOptions::default())
}

/// Reload a saved package string back into an EPackage.
fn reload(xml: &str) -> EPackage {
    load_ecore_package(xml).unwrap()
}

/// 1) A Java-style `.ecore` parses into the EPackage structure.
#[test]
fn load_java_style_ecore_structure_parsed() {
    let pkg = load(K_JAVA_STYLE_ECORE);
    assert_eq!(pkg.name(), "library");
    assert_eq!(
        pkg.ns_uri().unwrap().to_string(),
        "http://example.com/e2e/equiv/library"
    );
    assert_eq!(pkg.ns_prefix(), "library");
    assert_eq!(pkg.classes().len(), 2);
}

/// 2) Saving back emits the Java-format elements.
#[test]
fn save_contains_java_format_elements() {
    let pkg = load(K_JAVA_STYLE_ECORE);
    let out = save(&pkg);
    assert!(out.contains("<ecore:EPackage"), "{out}");
    assert!(out.contains("xmlns:ecore="), "{out}");
    assert!(out.contains("xmlns:xmi="), "{out}");
    assert!(out.contains("xmlns:xsi="), "{out}");
    assert!(out.contains("xsi:type=\"ecore:EClass\""), "{out}");
    assert!(out.contains("xsi:type=\"ecore:EAttribute\""), "{out}");
    assert!(out.contains("xsi:type=\"ecore:EReference\""), "{out}");
}

/// 3) The save emits exactly one `<ecore:EPackage>` root (no extra wrapper).
#[test]
fn save_single_ecore_root() {
    let pkg = load(K_JAVA_STYLE_ECORE);
    let out = save(&pkg);
    let first = out.find("<ecore:EPackage");
    assert!(first.is_some(), "must open an EPackage: {out}");
    if let Some(p) = first {
        let second = out[p + 1..].find("<ecore:EPackage");
        assert!(second.is_none(), "exactly one EPackage wrapper: {out}");
    }
}

/// 4) Roundtrip preserves package metadata.
#[test]
fn roundtrip_package_metadata_preserved() {
    let a = load(K_JAVA_STYLE_ECORE);
    let b = reload(&save(&a));
    assert_eq!(b.name(), a.name());
    assert_eq!(b.ns_uri().map(|u| u.to_string()), a.ns_uri().map(|u| u.to_string()));
    assert_eq!(b.ns_prefix(), a.ns_prefix());
}

/// 5) Roundtrip preserves classifier count and names.
#[test]
fn roundtrip_classifiers_preserved() {
    let a = load(K_JAVA_STYLE_ECORE);
    let b = reload(&save(&a));
    assert_eq!(b.classes().len(), a.classes().len());
    assert!(b.find_class("Library").is_some());
    assert!(b.find_class("Book").is_some());
}

/// 6) Roundtrip preserves an EAttribute's eType (EString).
#[test]
fn roundtrip_attribute_etype_preserved() {
    let a = load(K_JAVA_STYLE_ECORE);
    let b = reload(&save(&a));
    let lib = b.find_class("Library").unwrap();
    let name = lib
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "name")
        .expect("Library.name feature");
    assert!(!name.is_reference(), "name is an attribute");
    assert_eq!(name.type_name().unwrap(), "EString");
}

/// 7) Roundtrip preserves an EReference's containment + target class.
#[test]
fn roundtrip_reference_containment_preserved() {
    let a = load(K_JAVA_STYLE_ECORE);
    let b = reload(&save(&a));
    let lib = b.find_class("Library").unwrap();
    let books = lib
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "books")
        .expect("Library.books feature");
    assert!(books.is_reference(), "books is a reference");
    assert!(books.is_containment(), "books is a containment reference");
    assert_eq!(books.type_name().unwrap(), "Book");
}

/// 8) Roundtrip preserves defaultValueLiteral.
#[test]
fn roundtrip_default_value_literal_preserved() {
    let a = load(K_JAVA_STYLE_ECORE);
    let b = reload(&save(&a));
    let book = b.find_class("Book").unwrap();
    let pages = book
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "pages")
        .expect("Book.pages feature");
    assert_eq!(pages.default_value_literal(), Some("0"));
}

/// 9) Two saves are byte-identical (idempotent).
#[test]
fn save_idempotent() {
    let pkg = load(K_JAVA_STYLE_ECORE);
    let out1 = save(&pkg);
    let out2 = save(&pkg);
    assert_eq!(out1, out2);
}

/// 10) The actual Java-produced sample file parses and saves back with all key
/// metadata retained (nsURI `http://example.com/e2e/library`).
#[test]
fn load_sample_file_save_back() {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/samples/multi/library.ecore");
    let src = std::fs::read_to_string(&path).unwrap();
    let pkg = load(&src);
    assert_eq!(pkg.name(), "library");
    assert_eq!(
        pkg.ns_uri().unwrap().to_string(),
        "http://example.com/e2e/library"
    );

    let out = save(&pkg);
    assert!(out.contains("name=\"library\""), "{out}");
    assert!(out.contains("nsURI=\"http://example.com/e2e/library\""), "{out}");
    assert!(out.contains("xsi:type=\"ecore:EClass\""), "{out}");
    assert!(out.contains("containment=\"true\""), "{out}");
}