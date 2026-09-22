//! C++ parity suite: `XMILoaderTests.cpp` contract.
//!
//! Ports the C++ `emf-xmi/tests/XMILoaderTests.cpp` assertions onto the Rust
//! `.ecore` loader (`emf_ecore_codegen::loader::load_ecore_package`): parsing
//! an `<ecore:EPackage>` document into an `EPackage` must preserve package
//! metadata, classifiers, structural features, bounds, eType hrefs (ecore
//! builtins vs same-package `#//` refs), enum literals, data types, flags
//! (abstract / interface / iD / containment / resolveProxies) and super-types.

use emf_ecore::{EPackage, EStructuralFeature};

fn load(xml: &str) -> EPackage {
    emf_ecore_codegen::loader::load_ecore_package(xml).unwrap()
}

const K_SIMPLE_ECORE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="simple" nsURI="http://example.com/simple" nsPrefix="sim">
  <eClassifiers xsi:type="ecore:EClass" name="Foo">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="label"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>"##;

/// 1) Load simple .ecore: EPackage name/nsURI/nsPrefix + 1 classifier.
#[test]
fn load_simple_epackage_metadata() {
    let pkg = load(K_SIMPLE_ECORE);
    assert_eq!(pkg.name(), "simple");
    assert_eq!(pkg.ns_uri().unwrap().to_string(), "http://example.com/simple");
    assert_eq!(pkg.ns_prefix(), "sim");
    assert_eq!(pkg.classes().len(), 1);
}

/// 2) Load simple .ecore: EClass + EAttribute present.
#[test]
fn load_simple_epackage_eclass_and_eattribute() {
    let pkg = load(K_SIMPLE_ECORE);
    let cls = pkg.find_class("Foo").expect("Foo class");
    assert_eq!(cls.name(), "Foo");
    let feats = cls.e_structural_features();
    assert_eq!(feats.len(), 1);
    let sf = &feats[0];
    assert_eq!(sf.name(), "label");
    assert!(!sf.is_reference());
}

/// 3) eType href resolution: ecore builtin EString.
#[test]
fn load_etype_resolution_ecore_builtin_estring() {
    let pkg = load(K_SIMPLE_ECORE);
    let cls = pkg.find_class("Foo").unwrap();
    let attr = &cls.e_structural_features()[0];
    assert_eq!(attr.type_name().unwrap(), "EString");
}

/// 4) eType href resolution: same-package `#//Book` reference.
#[test]
fn load_etype_resolution_same_package_reference() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="lib" nsURI="http://example.com/lib" nsPrefix="lib">
  <eClassifiers xsi:type="ecore:EClass" name="Library">
    <eStructuralFeatures xsi:type="ecore:EReference" name="books"
        upperBound="-1" eType="#//Book" containment="true"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Book">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="title"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let lib = pkg.find_class("Library").unwrap();
    let books = lib
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "books")
        .unwrap();
    assert_eq!(books.upper_bound(), -1);
    assert!(books.is_containment());
    assert_eq!(books.type_name().unwrap(), "Book");
}

/// 5) Load EEnum: eLiterals name/value/literal + lookup.
#[test]
fn load_eenum_with_literals() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="enums" nsURI="http://example.com/enums" nsPrefix="en">
  <eClassifiers xsi:type="ecore:EEnum" name="Color">
    <eLiterals name="Red" value="0" literal="RED"/>
    <eLiterals name="Green" value="1" literal="GREEN"/>
    <eLiterals name="Blue" value="2" literal="BLUE"/>
  </eClassifiers>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let en = pkg.find_enum("Color").expect("Color enum");
    let lits = en.e_literals();
    assert_eq!(lits.len(), 3);
    assert_eq!(lits[0].name(), "Red");
    assert_eq!(lits[0].value(), 0);
    assert_eq!(lits[0].literal(), "RED");
    assert_eq!(lits[2].value(), 2);
    assert_eq!(en.literal_by_name("Green").unwrap().literal(), "GREEN");
    assert_eq!(en.literal_by_value(1).unwrap().name(), "Green");
}

/// 6) Load EDataType: name + instanceClassName.
#[test]
fn load_edatatype() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="dt" nsURI="http://example.com/dt" nsPrefix="dt">
  <eClassifiers xsi:type="ecore:EDataType" name="Money"
      instanceClassName="java.math.BigDecimal"/>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let dt = pkg.find_data_type("Money").expect("Money datatype");
    assert_eq!(dt.name(), "Money");
    assert_eq!(dt.instance_class_name(), "java.math.BigDecimal");
}

/// 8) EClass flags: abstract / interface / instanceClassName.
#[test]
fn load_eclass_abstract_and_interface() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="abs" nsURI="http://example.com/abs" nsPrefix="ab">
  <eClassifiers xsi:type="ecore:EClass" name="Base"
      abstract="true" interface="true" instanceClassName="java.util.Collection"/>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let cls = pkg.find_class("Base").unwrap();
    assert!(cls.is_abstract());
    assert!(cls.is_interface());
    assert_eq!(cls.instance_class_name(), "java.util.Collection");
}

/// 9) EAttribute: bounds + defaultValueLiteral + iD.
#[test]
fn load_eattribute_bounds_default_id() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="b" nsURI="http://example.com/b" nsPrefix="b">
  <eClassifiers xsi:type="ecore:EClass" name="C">
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="id"
        iD="true" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="count"
        lowerBound="1" upperBound="5" defaultValueLiteral="0"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EInt"/>
  </eClassifiers>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let cls = pkg.find_class("C").unwrap();
    let feats: Vec<&EStructuralFeature> = cls.e_structural_features().iter().collect();
    let id_attr = feats[0].to_owned();
    assert!(id_attr.is_id());
    let count = feats[1].to_owned();
    assert_eq!(count.lower_bound(), 1);
    assert_eq!(count.upper_bound(), 5);
    assert_eq!(count.default_value_literal(), Some("0"));
}

/// 10) EReference: containment / resolveProxies / upperBound.
#[test]
fn load_ereference_containment_and_resolve_proxies() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="r" nsURI="http://example.com/r" nsPrefix="r">
  <eClassifiers xsi:type="ecore:EClass" name="Parent">
    <eStructuralFeatures xsi:type="ecore:EReference" name="kids"
        upperBound="-1" eType="#//Child" containment="true" resolveProxies="false"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Child"/>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let parent = pkg.find_class("Parent").unwrap();
    let kids = parent
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "kids")
        .unwrap();
    assert!(kids.is_containment());
    assert!(!kids.is_resolve_proxies());
    assert_eq!(kids.upper_bound(), -1);
    assert_eq!(kids.type_name().unwrap(), "Child");
}

/// 12) eSuperTypes same-package resolution + eAllSuperTypes.
#[test]
fn load_esupertypes_same_package() {
    const XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="inh" nsURI="http://example.com/inh" nsPrefix="inh">
  <eClassifiers xsi:type="ecore:EClass" name="Animal"/>
  <eClassifiers xsi:type="ecore:EClass" name="Dog" eSuperTypes="#//Animal"/>
</ecore:EPackage>"##;
    let pkg = load(XML);
    let dog = pkg.find_class("Dog").unwrap();
    assert_eq!(dog.e_super_types().len(), 1);
    assert_eq!(dog.e_super_types()[0], "Animal");
    // Registered package so reflection can resolve eAllSuperTypes.
    let mut reg = emf_ecore::PackageRegistry::new();
    reg.register(emf_ecore::make_package_ref(pkg));
    let dog2 = reg.find_class("Dog").unwrap();
    let all = dog2.e_all_super_types(&reg);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0], "Animal");
}