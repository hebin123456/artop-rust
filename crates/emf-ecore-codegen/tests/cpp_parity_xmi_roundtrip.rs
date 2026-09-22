//! C++ parity suite: `RoundtripTests.cpp` contract.
//!
//! Ports the C++ `emf-xmi/tests/RoundtripTests.cpp` assertions: load an
//! `.ecore` document into an `EPackage` (via the `emf-ecore-codegen` loader),
//! serialize it back out (via [`emf_xmi::metamodel_saver`]), reload, and prove
//! the package metadata / classifiers / features / types survive and that two
//! consecutive saves are byte-identical (idempotency).
//!
//! The loader is the metadata inverse of the saver; together they form the
//! `EPackage` round-trip path the C++ suite tests directly.

use emf_ecore::EPackage;
use emf_xmi::metamodel_saver::save_ecore_package;
use emf_xmi::options::XmiOptions;

const K_LIBRARY_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="library" nsURI="http://example.com/library/1.0" nsPrefix="library">
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
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"
        defaultValueLiteral="0"/>
  </eClassifiers>
</ecore:EPackage>"##;

const K_ENUM_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="enums" nsURI="http://example.com/enums/1.0" nsPrefix="en">
  <eClassifiers xsi:type="ecore:EEnum" name="Color">
    <eLiterals name="Red" value="0" literal="RED"/>
    <eLiterals name="Green" value="1" literal="GREEN"/>
  </eClassifiers>
</ecore:EPackage>"##;

fn load(xml: &str) -> EPackage {
    emf_ecore_codegen::loader::load_ecore_package(xml).unwrap()
}

fn save(pkg: &EPackage) -> String {
    save_ecore_package(pkg, &XmiOptions::default())
}

// ---------------------------------------------------------------------------
// 1) Roundtrip .ecore: name/nsURI/nsPrefix preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ecore_package_metadata_preserved() {
    let p1 = load(K_LIBRARY_ECORE);
    let saved = save(&p1);
    let p2 = load(&saved);
    assert_eq!(p2.name(), p1.name());
    assert_eq!(p2.ns_uri().map(|u| u.to_string()), p1.ns_uri().map(|u| u.to_string()));
    assert_eq!(p2.ns_prefix(), p1.ns_prefix());
    assert_eq!(p2.name(), "library");
    assert_eq!(p2.ns_prefix(), "library");
}

// ---------------------------------------------------------------------------
// 2) Roundtrip .ecore: classifier count and names preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ecore_classifiers_preserved() {
    let p1 = load(K_LIBRARY_ECORE);
    let saved = save(&p1);
    let p2 = load(&saved);
    assert_eq!(p2.classes().len(), p1.classes().len());
    assert_eq!(p2.classes().len(), 2);
    assert!(p2.find_class("Library").is_some());
    assert!(p2.find_class("Book").is_some());
}

// ---------------------------------------------------------------------------
// 3) Roundtrip .ecore: EClass features name/type preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ecore_features_and_types_preserved() {
    let p1 = load(K_LIBRARY_ECORE);
    let saved = save(&p1);
    let p2 = load(&saved);

    let lib = p2.find_class("Library").expect("Library");
    let feats = lib.e_structural_features();
    assert_eq!(feats.len(), 2);
    let name_attr = feats.iter().find(|f| f.name() == "name").unwrap();
    assert!(!name_attr.is_reference());
    assert_eq!(name_attr.type_name().unwrap(), "EString");
    let books_ref = feats.iter().find(|f| f.name() == "books").unwrap();
    assert!(books_ref.is_reference());
    assert_eq!(books_ref.upper_bound(), -1);
    assert!(books_ref.is_containment());
    assert_eq!(books_ref.type_name().unwrap(), "Book");
}

// ---------------------------------------------------------------------------
// 4) Roundtrip .ecore: defaultValueLiteral preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ecore_default_value_literal_preserved() {
    let p1 = load(K_LIBRARY_ECORE);
    let saved = save(&p1);
    let p2 = load(&saved);

    let book = p2.find_class("Book").expect("Book");
    let pages = book
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "pages")
        .unwrap();
    assert_eq!(pages.default_value_literal(), Some("0"));
    assert_eq!(pages.type_name().unwrap(), "EInt");
}

// ---------------------------------------------------------------------------
// 5) Roundtrip .ecore: two consecutive saves are idempotent (save1 == save2)
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ecore_save_is_idempotent() {
    let p1 = load(K_LIBRARY_ECORE);
    let saved1 = save(&p1);
    let p2 = load(&saved1);
    let saved2 = save(&p2);
    assert_eq!(saved1, saved2);
}

// ---------------------------------------------------------------------------
// 6) Roundtrip EEnum: literals name/value/literal preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_eenum_literals_preserved() {
    let p1 = load(K_ENUM_ECORE);
    let saved = save(&p1);
    let p2 = load(&saved);

    let en = p2.find_enum("Color").expect("Color enum found");
    let lits = en.e_literals();
    assert_eq!(lits.len(), 2);
    assert_eq!(lits[0].name(), "Red");
    assert_eq!(lits[0].value(), 0);
    assert_eq!(lits[0].literal(), "RED");
    assert_eq!(lits[1].name(), "Green");
    assert_eq!(lits[1].value(), 1);
}

// ---------------------------------------------------------------------------
// 7) Roundtrip: abstract flag preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ecore_abstract_flag_preserved() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="ab" nsURI="http://example.com/ab" nsPrefix="ab">
  <eClassifiers xsi:type="ecore:EClass" name="Base" abstract="true"/>
  <eClassifiers xsi:type="ecore:EClass" name="Derived" eSuperTypes="#//Base"/>
</ecore:EPackage>"##;

    let p1 = load(XML);
    let saved = save(&p1);
    assert!(saved.contains("abstract=\"true\""), "{saved}");

    let p2 = load(&saved);
    let base = p2.find_class("Base").unwrap();
    assert!(base.is_abstract());
    let derived = p2.find_class("Derived").unwrap();
    assert_eq!(derived.e_super_types().len(), 1);
    assert_eq!(derived.e_super_types()[0], "Base");
}

// ---------------------------------------------------------------------------
// 8) Roundtrip: empty EPackage save -> reload -> save idempotent
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_empty_package_idempotent() {
    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="empty" nsURI="http://example.com/empty" nsPrefix="em"/>"#;

    let p1 = load(XML);
    let s1 = save(&p1);
    let p2 = load(&s1);
    let s2 = save(&p2);
    assert_eq!(s1, s2);
    assert!(s1.contains("name=\"empty\""), "{s1}");
}

// ---------------------------------------------------------------------------
// 9) Roundtrip: iD and resolveProxies flags preserved
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_id_and_resolve_proxies_preserved() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="flags" nsURI="http://example.com/flags" nsPrefix="fl">
  <eClassifiers xsi:type="ecore:EClass" name="Parent">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="uid"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString" iD="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Child">
    <eStructuralFeatures xsi:type="ecore:EReference" name="parent" upperBound="-1"
        eType="#//Parent" containment="true" resolveProxies="false"/>
  </eClassifiers>
</ecore:EPackage>"##;

    let p1 = load(XML);
    let saved = save(&p1);
    assert!(saved.contains("iD=\"true\""), "{saved}");
    assert!(saved.contains("resolveProxies=\"false\""), "{saved}");

    let p2 = load(&saved);
    let parent = p2.find_class("Parent").unwrap();
    let uid = parent
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "uid")
        .unwrap();
    assert!(uid.is_id());
    let child = p2.find_class("Child").unwrap();
    let par = child
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "parent")
        .unwrap();
    assert!(!par.is_resolve_proxies());
    assert!(par.is_containment());
}