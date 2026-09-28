//! C++ parity suite: `emf-xcore` (Xcore DSL parser + generator + resource).
//!
//! Ports the 14 `EMF_TEST`s of `artop-cpp/cpp/emf-cpp/emf-xcore/tests/
//! XcoreTests.cpp` onto the Rust `emf-xcore` crate. Each `#[test]` mirrors one
//! `EMF_TEST` of the same name, and the mapping is recorded in
//! `tools/conformance/cases_xcore.tsv`.
//!
//! ## Documented differences (tracked in `docs/PARITY_TRACKER.md`)
//!
//! - C++ `EPackage` is an `EObject`, so `XcoreResource::getContents()` yields
//!   the derived package as an `EObject*`. In this port `EPackage` is a value
//!   type (`PackageRef`), not an `EObject`, so [`XcoreResource::contents`]
//!   returns a one-element `Vec<PackageRef>`.
//! - C++ resolves an attribute's `eType` by pointer and the tests read
//!   `getEAttributeType()->getName()`. The Rust generator stores the equivalent
//!   classifier *name* on the feature, so the type-mapping test asserts
//!   [`EStructuralFeature::type_name`] (`"EString"`, `"EInt"`, ...).
//! - C++ `XcoreResource::load` registers the derived package in the global
//!   `EPackageRegistry`. This port's registry is a per-thread snapshot, so the
//!   end-to-end test asserts on the resource's derived package directly.
//! - C++ `EOperation` exposes `body_` only through the impl; neither side
//!   asserts on the operation body.

use emf_common::uri::Uri;
use emf_ecore::PackageRegistry;
use emf_xcore::{
    parse, ReferenceKind, XcoreGenerator, XcoreResource, XcoreResourceFactory, XcoreStandaloneSetup,
};

/// Parse an `.xcore` source and return its package AST (aborts on error).
fn pkg_of(src: &str) -> emf_xcore::PackageDecl {
    parse(src)
        .expect("parse ok")
        .package
        .expect("package present")
}

// ===========================================================================
// XcoreParserTests (1–3)
// ===========================================================================

#[test]
fn xcore_parser_basic_class() {
    let src = r#"
        package demo
        class Foo {
            String name
            int count
            contains Bar[] bars
            op String greet() { "hi" }
        }
        class Bar {
            boolean active
        }
    "#;
    let pkg = pkg_of(src);
    assert_eq!(pkg.name, "demo");
    assert_eq!(pkg.classes.len(), 2);
    let foo = &pkg.classes[0];
    assert_eq!(foo.name, "Foo");
    assert_eq!(foo.attributes.len(), 2);
    assert_eq!(foo.references.len(), 1);
    assert_eq!(foo.operations.len(), 1);
    assert_eq!(foo.references[0].type_name, "Bar");
    assert_eq!(foo.references[0].kind, ReferenceKind::Containment);
    assert!(foo.references[0].multi);
    assert_eq!(foo.operations[0].name, "greet");
    assert_eq!(foo.operations[0].type_name, "String");
}

#[test]
fn xcore_parser_extends_and_enum() {
    let src = r#"
        package example
        class Base { String id }
        class Derived extends Base {
            refers Base parent
        }
        enum Color { RED = 0, GREEN, BLUE }
    "#;
    let pkg = pkg_of(src);
    assert_eq!(pkg.classes.len(), 2);
    assert_eq!(pkg.classes[1].super_types.len(), 1);
    assert_eq!(pkg.classes[1].super_types[0], "Base");
    assert_eq!(pkg.enums.len(), 1);
    let e = &pkg.enums[0];
    assert_eq!(e.literals.len(), 3);
    assert_eq!(e.literals[0].name, "RED");
    assert_eq!(e.literals[0].value, Some(0));
    assert_eq!(e.literals[1].value, Some(1)); // auto-increment
    assert_eq!(e.literals[2].value, Some(2));
}

#[test]
fn xcore_parser_annotations_and_modifiers() {
    let src = r#"
        @Ecore(nsURI="http://test", nsPrefix="t")
        package test
        annotation "http://www.eclipse.org/emf/2002/Ecore" as Ecore
        class Node {
            @Ecore(name="NODE")
            derived long average get { 0 }
            readonly String label
            id String uuid
        }
    "#;
    let pkg = pkg_of(src);
    assert_eq!(pkg.ns_uri, "http://test");
    assert_eq!(pkg.ns_prefix, "t");
    assert_eq!(pkg.annotation_directives.len(), 1);
    let node = &pkg.classes[0];
    assert_eq!(node.attributes.len(), 3);
    // average: derived, get body
    assert!(node.attributes[0].derived);
    assert!(node.attributes[0].getter_body.is_some());
    // label: readonly
    assert!(node.attributes[1].read_only);
    // uuid: id
    assert!(node.attributes[2].id);
}

// ===========================================================================
// XcoreGeneratorTests (4–6, 11–14)
// ===========================================================================

#[test]
fn xcore_generator_derive_epackage() {
    let src = r#"
        package demo
        class Foo {
            String name
            int count
            contains Bar[] bars
            op String greet() { "hi" }
        }
        class Bar { boolean active }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    assert_eq!(p.name(), "demo");
    // classifiers: 2 classes
    assert_eq!(p.classes().len(), 2);
    let foo = p.find_class("Foo").expect("Foo");
    let attr_count = foo
        .e_structural_features()
        .iter()
        .filter(|f| !f.is_reference())
        .count();
    let ref_count = foo
        .e_structural_features()
        .iter()
        .filter(|f| f.is_reference())
        .count();
    assert_eq!(attr_count, 2);
    assert_eq!(ref_count, 1);
    assert_eq!(foo.e_operations().len(), 1);
    // bars: containment, multi
    let bars = foo
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "bars")
        .expect("bars");
    assert!(bars.is_containment());
    assert_eq!(bars.upper_bound(), -1);
    // greet operation, no parameters
    let greet = foo
        .e_operations()
        .iter()
        .find(|o| o.name() == "greet")
        .expect("greet");
    assert_eq!(greet.parameters().len(), 0);
}

#[test]
fn xcore_generator_inheritance() {
    let src = r#"
        package demo
        class Base { String id }
        class Derived extends Base { String extra }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);

    let mut reg = PackageRegistry::new();
    reg.register(pkg.clone());
    let p = pkg.borrow();
    let derived = p.find_class("Derived").expect("Derived");
    // eSuperTypes
    assert_eq!(derived.e_super_types().len(), 1);
    assert_eq!(derived.e_super_types()[0], "Base");
    // eAllSuperTypes recursive
    assert_eq!(derived.e_all_super_types(&reg).len(), 1);
    // eAllStructuralFeatures: Base.id + Derived.extra == 2
    assert_eq!(derived.e_all_structural_features(&reg).len(), 2);
}

#[test]
fn xcore_generator_type_mapping() {
    let src = r#"
        package demo
        class Types {
            String s
            boolean b
            int i
            long l
            short h
            double d
            float f
            char c
            byte by
        }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    let types = p.find_class("Types").expect("Types");
    let attrs: Vec<&str> = types
        .e_structural_features()
        .iter()
        .filter(|f| !f.is_reference())
        .filter_map(|f| f.type_name())
        .collect();
    assert_eq!(attrs.len(), 9);
    // String → EString, boolean → EBoolean, int → EInt, long → ELong
    assert_eq!(attrs[0], "EString");
    assert_eq!(attrs[1], "EBoolean");
    assert_eq!(attrs[2], "EInt");
    assert_eq!(attrs[3], "ELong");
}

#[test]
fn xcore_generator_e_operation_completeness() {
    // Aligned to Java XcoreEOperationBuilder: an `op` sets eType + eParameters.
    let src = r#"
        package demo
        class Calculator {
            op int add(int a, int b) { a + b }
            op String format(int value) { "" }
            op boolean check() throws Exception { true }
        }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    let calc = p.find_class("Calculator").expect("Calculator");
    assert_eq!(calc.e_operations().len(), 3);

    // add(int a, int b) : int
    let add = calc
        .e_operations()
        .iter()
        .find(|o| o.name() == "add")
        .expect("add");
    assert_eq!(add.return_type(), Some("EInt"));
    assert_eq!(add.parameters().len(), 2);
    assert_eq!(add.parameters()[0].name(), "a");
    assert_eq!(add.parameters()[0].type_name(), Some("EInt"));
    assert_eq!(add.parameters()[1].name(), "b");
    assert_eq!(add.parameters()[1].type_name(), Some("EInt"));

    // format(int value) : String
    let fmt = calc
        .e_operations()
        .iter()
        .find(|o| o.name() == "format")
        .expect("format");
    assert_eq!(fmt.return_type(), Some("EString"));
    assert_eq!(fmt.parameters().len(), 1);
    assert_eq!(fmt.parameters()[0].name(), "value");

    // check() : boolean throws Exception
    let chk = calc
        .e_operations()
        .iter()
        .find(|o| o.name() == "check")
        .expect("check");
    assert_eq!(chk.return_type(), Some("EBoolean"));
    assert_eq!(chk.parameters().len(), 0);
}

#[test]
fn xcore_generator_e_opposite_bidirectional() {
    // Aligned to Java EReference.setEOpposite: opposite ends cross-link.
    let src = r#"
        package demo
        class Parent {
            contains Child[] children opposite parent
        }
        class Child {
            refers Parent parent opposite children
        }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    let parent = p.find_class("Parent").expect("Parent");
    let child = p.find_class("Child").expect("Child");

    let children = parent
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "children")
        .expect("children");
    let parent_ref = child
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "parent")
        .expect("parent");

    // Bidirectional: children.eOpposite == parent && parent.eOpposite == children
    assert_eq!(children.opposite(), Some("parent"));
    assert_eq!(parent_ref.opposite(), Some("children"));

    // contains semantics retained: children is containment, parent is not
    assert!(children.is_containment());
    assert!(!parent_ref.is_containment());
}

#[test]
fn xcore_generator_e_annotation_propagation() {
    // Aligned to Java XcoreEAnnotationBuilder: `@Directive(k=v)` propagates to
    // a derived EAnnotation whose source is the directive's URI.
    let src = r#"
        @Ecore(nsURI="http://annot", nsPrefix="a")
        package annot

        annotation "http://www.eclipse.org/emf/2002/Ecore" as Ecore
        annotation "http:///org/eclipse/emf/ecore/util/ExtendedMetaData" as ExtendedMetaData

        @ExtendedMetaData(name="NODE")
        class Node {
            @Ecore(name="NODE_ATTR")
            String label
        }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    let node = p.find_class("Node").expect("Node");

    // Class-level annotation: @ExtendedMetaData(name="NODE")
    let cls_ann = node
        .e_annotations()
        .iter()
        .find(|a| a.source() == "http:///org/eclipse/emf/ecore/util/ExtendedMetaData")
        .expect("class annotation");
    assert_eq!(cls_ann.detail("name"), Some("NODE"));

    // Member-level annotation: @Ecore(name="NODE_ATTR")
    let label = node
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "label")
        .expect("label");
    let attr_ann = label
        .e_annotations()
        .iter()
        .find(|a| a.source() == "http://www.eclipse.org/emf/2002/Ecore")
        .expect("attribute annotation");
    assert_eq!(attr_ann.detail("name"), Some("NODE_ATTR"));
}

#[test]
fn xcore_generator_gen_model_generation() {
    // Aligned to Java GenModel serialization.
    let src = r#"
        package demo
        class Foo {
            String name
            contains Bar[] bars
            op String greet(String who) { "" }
        }
        class Bar { boolean active }
        enum Color { RED, GREEN }
    "#;
    let xpkg = pkg_of(src);
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    assert_eq!(pkg.borrow().name(), "demo");

    let gm = gen.generate_gen_model(&xpkg);
    // key elements present
    assert!(gm.contains("<genmodel:GenModel"));
    assert!(gm.contains("modelDirectory=\"/src\""));
    assert!(gm.contains("complianceLevel=\"8.0\""));
    assert!(gm.contains("<foreignModel>demo.xcore</foreignModel>"));
    assert!(gm.contains("<genPackages"));
    assert!(gm.contains("ecorePackage=\"demo#/\""));
    // genClasses Foo
    assert!(gm.contains("ecoreClass=\"Foo\""));
    // genFeatures: name (attribute) + bars (reference)
    assert!(gm.contains("ecore:EAttribute name"));
    assert!(gm.contains("ecore:EReference bars"));
    // genOperations greet with genParameters who
    assert!(gm.contains("ecoreOperation=\"greet\""));
    assert!(gm.contains("ecoreParameter=\"who\""));
    // genEnums Color with genEnumLiterals
    assert!(gm.contains("ecoreEnum=\"Color\""));
    assert!(gm.contains("ecoreEnumLiteral=\"RED\""));

    // XcoreResource.getGenModel() is also non-empty
    let mut res = XcoreResource::new(Uri::parse("demo.xcore"));
    res.load_str(src).expect("load ok");
    assert!(!res.gen_model().is_empty());
    assert!(res.gen_model().contains("genmodel:GenModel"));
}

// ===========================================================================
// XcoreResourceTests (7, 10)
// ===========================================================================

#[test]
fn xcore_resource_load_from_string() {
    let src = r#"
        package demo
        class Foo { String name }
    "#;
    let uri = Uri::parse("demo.xcore");
    let mut res = XcoreResource::new(uri);
    res.load_str(src).expect("load ok");
    let pkg = res.package().expect("derived package");
    assert_eq!(pkg.borrow().name(), "demo");
    // contents()[0] is the derived EPackage
    assert_eq!(res.contents().len(), 1);
}

#[test]
fn xcore_standalone_setup_registers() {
    XcoreStandaloneSetup::setup();
    // idempotent
    XcoreStandaloneSetup::setup();
    // factory creates a resource for a .xcore URI
    let uri = Uri::parse("test.xcore");
    let r = XcoreResourceFactory::create_resource_for(&uri);
    assert!(r.is_some());
}

// ===========================================================================
// XcoreRealSampleTests (8–9)
// ===========================================================================

#[test]
fn xcore_real_sample_design_principles() {
    let src = r#"
package example

/**
 * Class Node
 */
class ClassA {
        String attributeA
        String[] attributeB
        contains ClassB referenceA
        contains ClassB[] referenceB
        refers ClassC referenceC
        refers ClassC[] referenceD
}

class ClassB {
        String attributeC
}


class SubClassB extends ClassB {
        String[] attributeD
}


class ClassC {
        String attributeF
}

class SubClassC extends ClassC {
        String[] attributeG
}
"#;
    let xpkg = pkg_of(src);
    assert_eq!(xpkg.name, "example");
    assert_eq!(xpkg.classes.len(), 5);
    // ClassA: 2 attr, 4 ref
    let a = &xpkg.classes[0];
    assert_eq!(a.name, "ClassA");
    assert_eq!(a.attributes.len(), 2);
    assert_eq!(a.references.len(), 4);
    assert_eq!(a.references[0].kind, ReferenceKind::Containment);
    assert_eq!(a.references[2].kind, ReferenceKind::NonContainment);
    assert!(a.attributes[1].multi);
    // SubClassB extends ClassB
    assert_eq!(xpkg.classes[2].super_types.len(), 1);
    assert_eq!(xpkg.classes[2].super_types[0], "ClassB");

    // Generator derives
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    assert_eq!(p.classes().len(), 5);
    let sub = p.find_class("SubClassB").expect("SubClassB");
    assert_eq!(sub.e_super_types().len(), 1);
}

#[test]
fn xcore_real_sample_e_attribute_contained() {
    // Real sample carrying annotation directives.
    let src = r#"
@Ecore(nsURI="nodesURI")
package nodes

annotation "http://www.eclipse.org/emf/2002/Ecore"
as Ecore

annotation "http:///org/eclipse/emf/ecore/util/ExtendedMetaData"
as ExtendedMetaData

/**
 * Class Node
 */
@ExtendedMetaData(name="NODE")
class Node {
        @ExtendedMetaData(name="NODE")
        String[] property
}


/**
 * Datatype String
 */
@ExtendedMetaData(name="STRING")
type String wraps java.lang.String
"#;
    let xpkg = pkg_of(src);
    assert_eq!(xpkg.name, "nodes");
    assert_eq!(xpkg.ns_uri, "nodesURI");
    assert_eq!(xpkg.annotation_directives.len(), 2);
    assert_eq!(xpkg.classes.len(), 1);
    assert_eq!(xpkg.data_types.len(), 1);
    assert_eq!(xpkg.data_types[0].name, "String");
    assert_eq!(xpkg.data_types[0].wrapped_class_name, "java.lang.String");

    // Derive: 1 class + 1 datatype == 2 classifiers
    let gen = XcoreGenerator::new();
    let pkg = gen.generate(&xpkg);
    let p = pkg.borrow();
    assert_eq!(p.classes().len() + p.data_types().len(), 2);
    assert!(p.find_class("Node").is_some());
}
