//! C++ parity suite for static modeling.
//!
//! Ports the assertions of artop-cpp's
//! `cpp/emf-cpp/emf-ecore-codegen/tests/GenModelLoaderTests.cpp` and the
//! model-construction half of `tests/RuntimeBehaviorTests.cpp` onto the Rust
//! `emf-ecore-codegen` pipeline.
//!
//! The `.ecore` under test is the **same byte-identical file** that the C++
//! tests load (`emf-ecore-codegen/tests/samples/library.ecore`), so the facts
//! asserted here correspond one-to-one to the C++ expectations.

use std::path::{Path, PathBuf};

use emf_ecore::{DynamicEObject, EClass, EPackage, PackageRegistry};
use emf_ecore_codegen::GenModel;

fn sample_ecore() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/library.ecore")
}

fn model() -> GenModel {
    GenModel::load_path(sample_ecore()).unwrap()
}

fn load_ecore(xml: &str) -> EPackage {
    emf_ecore_codegen::loader::load_ecore_package(xml).unwrap()
}

// ---------------------------------------------------------------------------
//  GenModelLoaderTests.cpp :: wrapEcore — EPackage -> GenModel metadata
// ---------------------------------------------------------------------------

/// C++ `GenModelLoader_wrapEcore_buildsGenPackage`.
/// 3 EClasses (Library/Book/Writer); Library features: `name` (attribute,
/// EString) and `books` (many containment reference to Book).
#[test]
fn wrap_ecore_builds_package_metadata() {
    let m = model();
    assert_eq!(m.package_name(), "library");
    let pkg = m.package();
    assert_eq!(pkg.classes().len(), 3);
    for name in ["Library", "Book", "Writer"] {
        assert!(pkg.find_class(name).is_some(), "missing class {name}");
    }

    let lib = pkg.find_class("Library").unwrap();
    let feats = lib.e_structural_features();
    assert_eq!(feats.len(), 2);

    let name = feats.iter().find(|f| f.name() == "name").unwrap();
    assert!(!name.is_reference(), "name is an EAttribute");
    assert_eq!(name.type_name().unwrap(), "EString");

    let books = feats.iter().find(|f| f.name() == "books").unwrap();
    assert!(books.is_reference(), "books is an EReference");
    assert!(books.is_containment(), "books is containment");
    assert!(books.is_many(), "books is multi-valued");
    assert_eq!(books.type_name().unwrap(), "Book");
}

/// C++ `GenModelLoader_wrapEcore_recognizesReference`.
/// `author` is a single-valued containment EReference to Writer; `title` /
/// `pages` are attributes.
#[test]
fn wrap_ecore_recognizes_reference() {
    let m = model();
    let pkg = m.package();
    let book = pkg.find_class("Book").unwrap();
    let feats = book.e_structural_features();
    assert_eq!(feats.len(), 3);

    let author = feats.iter().find(|f| f.name() == "author").unwrap();
    assert!(author.is_reference());
    assert!(author.is_containment());
    assert!(!author.is_many(), "author is single-valued");
    assert_eq!(author.type_name().unwrap(), "Writer");

    let title = feats.iter().find(|f| f.name() == "title").unwrap();
    assert!(!title.is_reference());
    assert_eq!(title.type_name().unwrap(), "EString");

    let pages = feats.iter().find(|f| f.name() == "pages").unwrap();
    assert!(!pages.is_reference());
    assert_eq!(pages.type_name().unwrap(), "EInt");
    assert_eq!(pages.default_value_literal(), Some("0"));
}

/// C++ `GenModelLoader_loadFromString_parsesGenFeatures` (type mapping):
/// EString -> "EString", EInt -> "EInt", reference target -> class name.
#[test]
fn feature_type_names_match_cpp() {
    let m = model();
    let pkg = m.package();
    let writer = pkg.find_class("Writer").unwrap();
    let name = writer.e_structural_features()[0].clone();
    assert!(!name.is_reference());
    assert_eq!(name.type_name().unwrap(), "EString");
}

// ---------------------------------------------------------------------------
//  RuntimeBehaviorTests.cpp :: dynamic EObject reflection on the loaded model
// ---------------------------------------------------------------------------

fn book_class() -> EClass {
    model().package().find_class("Book").unwrap().clone()
}

/// Runtime_RDynamicEObject_eClass_ReturnsCorrectClass.
#[test]
fn dynamic_eclass_returns_class() {
    let cls = book_class();
    let obj = DynamicEObject::new(cls.clone());
    assert_eq!(obj.class().name(), "Book");
}

/// Runtime_RDynamicEObject_eSet_eGet_SingleAttribute + eSet_OverwritesValue.
#[test]
fn dynamic_set_get_and_overwrite() {
    let mut obj = DynamicEObject::new(book_class());
    assert!(obj.e_set_by_name("title", emf_ecore::Val::String("First".into())));
    assert_eq!(
        obj.e_get_by_name("title"),
        Some(emf_ecore::Val::String("First".into()))
    );
    obj.e_set_by_name("title", emf_ecore::Val::String("Second".into()));
    assert_eq!(
        obj.e_get_by_name("title"),
        Some(emf_ecore::Val::String("Second".into()))
    );
}

/// Runtime_RDynamicEObject_eIsSet_BeforeAndAfterSet + eUnset_ClearsIsSet.
#[test]
fn dynamic_isset_and_unset() {
    let mut obj = DynamicEObject::new(book_class());
    assert_eq!(obj.e_is_set_by_name("title"), Some(false));
    obj.e_set_by_name("title", emf_ecore::Val::String("A Title".into()));
    assert_eq!(obj.e_is_set_by_name("title"), Some(true));
    obj.e_unset_by_name("title");
    assert_eq!(obj.e_is_set_by_name("title"), Some(false));
}

/// The loaded model registers into a PackageRegistry for reflection lookup.
#[test]
fn loads_and_registers_into_registry() {
    let m = model();
    let mut reg = PackageRegistry::new();
    m.register(&mut reg);
    assert_eq!(reg.find_class("Book").unwrap().name(), "Book");
    assert_eq!(
        reg.find_class("Library")
            .unwrap()
            .e_structural_features()
            .len(),
        2
    );
}

// ---------------------------------------------------------------------------
//  RuntimeBehaviorTests.cpp :: P0-2 metamodel-element preservation on load
// ---------------------------------------------------------------------------

const K_ECORE_WITH_OP_AND_ANN: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="srv" nsURI="http://example.com/srv/1.0" nsPrefix="srv">
  <eClassifiers xsi:type="ecore:EClass" name="Service">
    <eAnnotations source="http://example.com/doc">
      <details key="author" value="emf"/>
      <details key="version" value="2"/>
    </eAnnotations>
    <eOperations name="invoke" lowerBound="0" upperBound="1" eType="#//Result">
      <eParameters name="request" eType="#//Request"/>
    </eOperations>
    <eStructuralFeatures xsi:type="ecore:EAttribute" name="id"
        eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
  </eClassifiers>
  <eClassifiers xsi:type="ecore:EClass" name="Result"/>
  <eClassifiers xsi:type="ecore:EClass" name="Request"/>
</ecore:EPackage>"##;

/// Runtime_LoadEcore_EOperationAndAnnotation_Preserved: loading must not drop
/// `EOperation` / `EParameter` / `EAnnotation` (Java SAXXMIHandler parity).
#[test]
fn load_ecore_eoperation_and_annotation_preserved() {
    let pkg = load_ecore(K_ECORE_WITH_OP_AND_ANN);
    assert!(pkg.find_class("Service").is_some(), "Service class");

    // EOperation (via eAllOperations; Service has no supertypes).
    let mut reg = PackageRegistry::new();
    reg.register(emf_ecore::make_package_ref(pkg));
    let svc = reg.find_class("Service").unwrap();
    let ops = svc.e_all_operations(&reg);
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].name(), "invoke");
    assert_eq!(ops[0].return_type(), Some("Result"));

    // EParameter with resolved type.
    let params = ops[0].parameters();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name(), "request");
    assert_eq!(params[0].type_name(), Some("Request"));

    // EAnnotation with details.
    let anns = svc.e_annotations();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].source(), "http://example.com/doc");
    assert_eq!(anns[0].detail("author"), Some("emf"));
    assert_eq!(anns[0].detail("version"), Some("2"));

    // EStructuralFeature still parses alongside the new builders.
    assert!(svc.e_structural_features().iter().any(|f| f.name() == "id"));
}

const K_ECORE_GENERIC: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ecore:EPackage xmi:version="2.0"
    xmlns:xmi="http://www.omg.org/XMI"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore"
    name="gen" nsURI="http://example.com/gen/1.0" nsPrefix="gen">
  <eClassifiers xsi:type="ecore:EClass" name="Base"/>
  <eClassifiers xsi:type="ecore:EClass" name="Container">
    <eTypeParameters name="T"/>
    <eGenericSuperTypes eClassifier="#//Base"/>
  </eClassifiers>
</ecore:EPackage>"##;

/// Runtime_LoadEcore_TypeParameterAndGenericSuper_Preserved: `eTypeParameters`
/// and `eGenericSuperTypes` (derived into `eSuperTypes`).
#[test]
fn load_ecore_type_parameter_and_generic_super_preserved() {
    let pkg = load_ecore(K_ECORE_GENERIC);
    let cont = pkg.find_class("Container").expect("Container class");

    let tps = cont.e_type_parameters();
    assert_eq!(tps.len(), 1);
    assert_eq!(tps[0].name(), "T");

    // eGenericSuperTypes -> eSuperTypes derived path.
    assert_eq!(cont.e_super_types().len(), 1);
    assert_eq!(cont.e_super_types()[0], "Base");
}
