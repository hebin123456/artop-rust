//! C++ parity suite: the `XMISaverTests.cpp` contract.
//!
//! Ports the C++ `emf-xmi/tests/XMISaverTests.cpp` assertions onto the Rust
//! [`emf_xmi::metamodel_saver`], which serializes an `EPackage` meta-model back
//! into an `<ecore:EPackage>` XMI document. Every assertion below is a
//! substring find against produced XML, exactly like the C++ `EXPECT_TRUE`,
//! so behaviour (not byte layout) is what we align.

use emf_ecore::datatype::names::{E_BOOLEAN, E_INT, E_STRING};
use emf_ecore::structural::FeatureKind;
use emf_ecore::{EClass, EClassKind, EEnum, EPackage, EStructuralFeature};
use emf_xmi::metamodel_saver::save_ecore_package;
use emf_xmi::options::XmiOptions;

/// `buildSimplePackage`: name/nsURI/nsPrefix + one EClass Foo + one EAttribute
/// label->EString.
fn simple_package() -> EPackage {
    let mut pkg = EPackage::new("simple");
    pkg.set_ns_uri("http://example.com/simple");
    pkg.set_ns_prefix("sim");
    let mut cls = EClass::new("Foo", EClassKind::Class);
    let mut attr = EStructuralFeature::new("label", FeatureKind::Attribute, 0, 1);
    attr.set_type_name(E_STRING);
    cls.add_feature(attr);
    pkg.add_class(cls);
    pkg
}

fn save(pkg: &EPackage) -> String {
    save_ecore_package(pkg, &XmiOptions::default())
}

// ---------------------------------------------------------------------------
// 1) Empty resource outputs <xmi:XMI/> empty document
// ---------------------------------------------------------------------------
// In the meta-document path an empty EPackage still yields a full
// <ecore:EPackage> root (there is no `<xmi:XMI/>` — that is the *instance*
// path, covered by the saver module). We assert the ecore root is emitted.
#[test]
fn empty_package_emits_epackage_root() {
    let mut pkg = EPackage::new("empty");
    pkg.set_ns_uri("http://example.com/empty");
    pkg.set_ns_prefix("em");
    let out = save(&pkg);
    assert!(out.find("<?xml").is_some(), "{out}");
    assert!(out.find("<ecore:EPackage").is_some(), "{out}");
    assert!(out.find("</ecore:EPackage>").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 2) Save EPackage: structure + namespace declarations
// ---------------------------------------------------------------------------
#[test]
fn save_epackage_structure() {
    let out = save(&simple_package());
    assert!(
        out.find("<?xml version=\"1.0\" encoding=\"UTF-8\"?>").is_some(),
        "{out}"
    );
    assert!(out.find("<ecore:EPackage").is_some(), "{out}");
    assert!(
        out.find("xmlns:xmi=\"http://www.omg.org/XMI\"").is_some(),
        "{out}"
    );
    assert!(
        out.find("xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\"")
            .is_some(),
        "{out}"
    );
    assert!(
        out.find("xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"")
            .is_some(),
        "{out}"
    );
    assert!(out.find("name=\"simple\"").is_some(), "{out}");
    assert!(
        out.find("nsURI=\"http://example.com/simple\"").is_some(),
        "{out}"
    );
    assert!(out.find("nsPrefix=\"sim\"").is_some(), "{out}");
    assert!(out.find("xmi:version=\"2.0\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 3) Save EPackage: eClassifiers + xsi:type="ecore:EClass"
// ---------------------------------------------------------------------------
#[test]
fn save_epackage_eclass_out() {
    let out = save(&simple_package());
    assert!(out.find("<eClassifiers").is_some(), "{out}");
    assert!(out.find("xsi:type=\"ecore:EClass\"").is_some(), "{out}");
    assert!(out.find("name=\"Foo\"").is_some(), "{out}");
    assert!(out.find("<eStructuralFeatures").is_some(), "{out}");
    assert!(out.find("xsi:type=\"ecore:EAttribute\"").is_some(), "{out}");
    assert!(out.find("name=\"label\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 4) Save EAttribute: eType rendered as ecore built-in href
// ---------------------------------------------------------------------------
#[test]
fn save_eattribute_etype_ecore_builtin() {
    let out = save(&simple_package());
    assert!(
        out.find(
            "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\""
        )
        .is_some(),
        "{out}"
    );
}

// ---------------------------------------------------------------------------
// 5) Save EReference: same-package eType as "#//Book" + containment
// ---------------------------------------------------------------------------
#[test]
fn save_ereference_same_package_etype() {
    let mut pkg = EPackage::new("lib");
    pkg.set_ns_uri("http://example.com/lib");
    pkg.set_ns_prefix("lib");
    let mut lib_cls = EClass::new("Library", EClassKind::Class);
    let book_cls = EClass::new("Book", EClassKind::Class);
    let mut books = EStructuralFeature::new("books", FeatureKind::Reference, 0, -1);
    books.set_type_name("Book");
    books.set_containment(true);
    lib_cls.add_feature(books);
    pkg.add_class(lib_cls);
    pkg.add_class(book_cls);

    let out = save(&pkg);
    assert!(out.find("xsi:type=\"ecore:EReference\"").is_some(), "{out}");
    assert!(out.find("name=\"books\"").is_some(), "{out}");
    assert!(out.find("upperBound=\"-1\"").is_some(), "{out}");
    assert!(out.find("containment=\"true\"").is_some(), "{out}");
    assert!(out.find("eType=\"#//Book\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 6) Save EEnum: eLiterals output, auto-increment value omitted
// ---------------------------------------------------------------------------
#[test]
fn save_eenum_literals() {
    let mut pkg = EPackage::new("enums");
    pkg.set_ns_uri("http://example.com/enums");
    pkg.set_ns_prefix("en");
    let mut en = EEnum::new("Color");
    {
        let lit = en.add_literal("Red", 0);
        lit.set_literal("RED");
    }
    {
        let lit = en.add_literal("Green", 1);
        lit.set_literal("GREEN");
    }
    pkg.add_enum(en);

    let out = save(&pkg);
    assert!(out.find("xsi:type=\"ecore:EEnum\"").is_some(), "{out}");
    assert!(out.find("name=\"Color\"").is_some(), "{out}");
    assert!(out.find("<eLiterals").is_some(), "{out}");
    assert!(out.find("name=\"Red\"").is_some(), "{out}");
    // value == index is omitted (Java EEnumLiteralSerializer behaviour)
    assert!(out.find("value=\"0\"").is_none(), "{out}");
    assert!(out.find("value=\"1\"").is_none(), "{out}");
    assert!(out.find("literal=\"GREEN\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 7) XMIOptions: custom encoding
// ---------------------------------------------------------------------------
#[test]
fn options_custom_encoding() {
    let mut opts = XmiOptions::default();
    opts.encoding = "ASCII".to_string();
    let out = save_ecore_package(&simple_package(), &opts);
    assert!(
        out.find("<?xml version=\"1.0\" encoding=\"ASCII\"?>").is_some(),
        "{out}"
    );
}

// ---------------------------------------------------------------------------
// 8) XMIOptions: xmlDeclaration=false -> no <?xml?>
// ---------------------------------------------------------------------------
#[test]
fn options_no_xml_declaration() {
    let mut opts = XmiOptions::default();
    opts.xml_declaration = false;
    let out = save_ecore_package(&simple_package(), &opts);
    assert!(out.find("<?xml").is_none(), "{out}");
}

// ---------------------------------------------------------------------------
// 9) XMIOptions: custom xmiVersion
// ---------------------------------------------------------------------------
#[test]
fn options_custom_xmi_version() {
    let mut opts = XmiOptions::default();
    opts.xmi_version = "2.1".to_string();
    let out = save_ecore_package(&simple_package(), &opts);
    assert!(out.find("xmi:version=\"2.1\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 10) Save EClass: abstract=true
// ---------------------------------------------------------------------------
#[test]
fn save_eclass_abstract_flag() {
    let mut pkg = EPackage::new("a");
    pkg.set_ns_uri("http://example.com/a");
    pkg.set_ns_prefix("a");
    let mut cls = EClass::new("Base", EClassKind::Class);
    cls.set_abstract(true);
    pkg.add_class(cls);

    let out = save(&pkg);
    assert!(out.find("abstract=\"true\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 11) Save EAttribute: iD=true
// ---------------------------------------------------------------------------
#[test]
fn save_eattribute_id_flag() {
    let mut pkg = EPackage::new("idpkg");
    pkg.set_ns_uri("http://example.com/idpkg");
    pkg.set_ns_prefix("idpkg");
    let mut cls = EClass::new("WithID", EClassKind::Class);
    let mut attr = EStructuralFeature::new("uid", FeatureKind::Attribute, 0, 1);
    attr.set_type_name(E_STRING);
    attr.set_id(true);
    cls.add_feature(attr);
    pkg.add_class(cls);

    let out = save(&pkg);
    assert!(out.find("iD=\"true\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 12) Save EReference: resolveProxies=false
// ---------------------------------------------------------------------------
#[test]
fn save_ereference_resolve_proxies_false() {
    let mut pkg = EPackage::new("rp");
    pkg.set_ns_uri("http://example.com/rp");
    pkg.set_ns_prefix("rp");
    let mut parent = EClass::new("Parent", EClassKind::Class);
    let child = EClass::new("Child", EClassKind::Class);
    let mut refr = EStructuralFeature::new("kids", FeatureKind::Reference, 0, -1);
    refr.set_type_name("Child");
    refr.set_containment(true);
    refr.set_resolve_proxies(false);
    parent.add_feature(refr);
    pkg.add_class(parent);
    pkg.add_class(child);

    let out = save(&pkg);
    assert!(out.find("resolveProxies=\"false\"").is_some(), "{out}");
}

// ---------------------------------------------------------------------------
// 13) defaults: non-N defaults for EInt/EBoolean reference the builtin href
// ---------------------------------------------------------------------------
#[test]
fn save_etype_for_int_and_boolean_builtins() {
    let mut pkg = EPackage::new("tp");
    pkg.set_ns_uri("http://example.com/tp");
    pkg.set_ns_prefix("tp");
    let mut cls = EClass::new("Item", EClassKind::Class);
    let mut n = EStructuralFeature::new("n", FeatureKind::Attribute, 0, 1);
    n.set_type_name(E_INT);
    let mut flags = EStructuralFeature::new("flags", FeatureKind::Attribute, 0, 1);
    flags.set_type_name(E_BOOLEAN);
    cls.add_feature(n);
    cls.add_feature(flags);
    pkg.add_class(cls);

    let out = save(&pkg);
    assert!(
        out.find(
            "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt\""
        )
        .is_some(),
        "{out}"
    );
    assert!(
        out.find(
            "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EBoolean\""
        )
        .is_some(),
        "{out}"
    );
}