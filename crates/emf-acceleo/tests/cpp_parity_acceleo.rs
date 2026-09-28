//! C++ parity suite: `emf-acceleo` (Acceleo MTL / M2T engine).
//!
//! Ports the C++ test binaries `AcceleoTests.cpp` and `AlignmentTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-acceleo/tests/` onto the Rust
//! [`emf_acceleo`] engine. Each `#[test]` mirrors one `EMF_TEST` of the same
//! name, and the mapping is recorded in `tools/conformance/cases_acceleo.tsv`.
//!
//! ## Model representation
//!
//! The C++ engine navigates model objects through `EObject::eGet(featureName)`.
//! The Rust engine does the same through [`emf_common::EObject::e_get`], so the
//! tests build their models as reflective [`DynamicEObject`]s. Where the C++
//! tests instantiate `EClassImpl`/`EPackageImpl`/`EAttributeImpl` straight from
//! `EcoreImpls`, the Rust tests build the equivalent runtime graph from
//! `emf-ecore` `EClass` descriptors plus `DynamicEObject` instances (see the
//! `support` module). This mirrors the C++ `XcoreGenerator` derivation for the
//! alignment tests, driven by the Rust `emf-xcore` parser.
//!
//! ## Documented differences (tracked in `docs/PARITY_TRACKER.md`)
//!
//! - C++ `AlignmentTests` route through `XcoreGenerator` + `XcoreResource`; the
//!   Rust `emf-xcore` crate currently ports only the parser. The alignment
//!   tests therefore parse the `.xcore` source with `emf_xcore::parse` and build
//!   the same `EPackage`/`EClass`/`EAttribute` runtime graph directly.
//! - C++ xcore accepts a brace-less `package name` + sibling `class` blocks; the
//!   Rust parser uses the standard `package name { ... }` form. The same
//!   class/feature declarations are used here.

use emf_acceleo::{AcceleoService, EvalContext};
use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{node_to_object, DynNode, DynamicEObject, EClass, EClassKind, EStructuralFeature};
use std::cell::RefCell;
use std::rc::Rc;

// ===== reflective metamodel support =====

/// A reflective object of the given (metadata) class.
fn obj_of(class: &EClass) -> DynNode {
    Rc::new(RefCell::new(DynamicEObject::new(class.clone())))
}

/// Set a single-valued string feature.
fn set_str(o: &DynNode, feature: &str, value: &str) {
    o.borrow_mut()
        .e_set_by_name(feature, Val::String(value.to_string()));
}

/// Set a single-valued object reference.
fn set_obj(o: &DynNode, feature: &str, value: &DynNode) {
    o.borrow_mut()
        .e_set_by_name(feature, Val::Object(node_to_object(value)));
}

/// Set a multi-valued object reference.
fn set_objs(o: &DynNode, feature: &str, values: &[DynNode]) {
    let list: Vec<Val> = values
        .iter()
        .map(|n| Val::Object(node_to_object(n)))
        .collect();
    o.borrow_mut().e_set_by_name(feature, Val::List(list));
}

/// Metadata class for a named element (`name : EString`).
fn named_meta() -> EClass {
    let mut c = EClass::new("NamedElement", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    c.add_feature(name);
    c
}

/// Metadata class for `EClass` (`name`, `eStructuralFeatures`).
fn eclass_meta() -> EClass {
    let mut c = EClass::new("EClass", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    c.add_feature(name);
    let mut feats = EStructuralFeature::reference_many("eStructuralFeatures");
    feats.set_containment(true);
    c.add_feature(feats);
    c
}

/// Metadata class for `EPackage` (`name`, `eClassifiers`).
fn epackage_meta() -> EClass {
    let mut c = EClass::new("EPackage", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    c.add_feature(name);
    let mut classifiers = EStructuralFeature::reference_many("eClassifiers");
    classifiers.set_containment(true);
    c.add_feature(classifiers);
    c
}

/// Metadata class for `EAttribute` (`name`, `eAttributeType`).
fn eattribute_meta() -> EClass {
    let mut c = EClass::new("EAttribute", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    c.add_feature(name);
    let mut ty = EStructuralFeature::reference("eAttributeType");
    ty.set_type_name("EClassifier");
    c.add_feature(ty);
    c
}

/// Metadata class for `EDataType` (`name`).
#[allow(dead_code)] // kept for symmetry with the other `*_meta` helpers.
fn edatatype_meta() -> EClass {
    let mut c = EClass::new("EDataType", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    c.add_feature(name);
    c
}

/// A simple named model object (a stand-in for an `EClass`/`EClassifier`).
fn named_model(name: &str) -> DynNode {
    let o = obj_of(&named_meta());
    set_str(&o, "name", name);
    o
}

/// An `EClass`-like model object carrying a list of `EAttribute`-like features.
fn eclass_model(name: &str, features: &[DynNode]) -> DynNode {
    let o = obj_of(&eclass_meta());
    set_str(&o, "name", name);
    set_objs(&o, "eStructuralFeatures", features);
    o
}

/// An `EAttribute`-like model object with a named `eAttributeType`.
fn eattribute_model(name: &str, type_name: &str) -> DynNode {
    let o = obj_of(&eattribute_meta());
    set_str(&o, "name", name);
    let ty = named_model(type_name);
    set_obj(&o, "eAttributeType", &ty);
    o
}

/// An `EPackage`-like model object carrying a list of classifiers.
fn package_model(name: &str, classifiers: &[DynNode]) -> DynNode {
    let o = obj_of(&epackage_meta());
    set_str(&o, "name", name);
    set_objs(&o, "eClassifiers", classifiers);
    o
}

/// A fresh, empty output directory for `[file]` generation tests.
fn fresh_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("artop_rust_acceleo_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// ===== AcceleoTests.cpp =====

/// AcceleoParser_Module.
#[test]
fn acceleo_parser_module() {
    let src = r#"
[module gen(c : Class)]
[template public genClass(c : Class)]
class [c.name/] {
}
[/template]
[/module]
"#;
    let m = emf_acceleo::parse(src).expect("parse ok");
    assert_eq!(m.name, "gen");
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].name, "c");
    assert_eq!(m.templates.len(), 1);
    assert_eq!(m.templates[0].name, "genClass");
    assert_eq!(m.templates[0].params.len(), 1);
}

/// AcceleoParser_ExprBlock: `Text "Hello " | Expr c.name | Text "!"`.
#[test]
fn acceleo_parser_expr_block() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)]Hello [c.name/]![/template]
[/module]
"#;
    let m = emf_acceleo::parse(src).expect("parse ok");
    let tpl = &m.templates[0];
    assert_eq!(tpl.body.len(), 3);
}

/// AcceleoParser_ForAndIf.
#[test]
fn acceleo_parser_for_and_if() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)]
[for (a | c.attributes)]
[if (a.name = 'id')]ID[a.name/][else][a.name/][/if]
[/for]
[/template]
[/module]
"#;
    let m = emf_acceleo::parse(src).expect("parse ok");
    let tpl = &m.templates[0];
    let has_for = tpl
        .body
        .iter()
        .any(|b| matches!(&**b, emf_acceleo::Block::For(_)));
    assert!(has_for);
}

/// AcceleoParser_Query.
#[test]
fn acceleo_parser_query() {
    let src = r#"
[module t(c : Class)]
[query public double(x : String) : String = 'X' + x /]
[template public f(c : Class)][/template]
[/module]
"#;
    let m = emf_acceleo::parse(src).expect("parse ok");
    assert_eq!(m.queries.len(), 1);
    assert_eq!(m.queries[0].name, "double");
    assert_eq!(m.queries[0].return_type_name, "String");
}

/// AcceleoEngine_StaticText.
#[test]
fn acceleo_engine_static_text() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)]Hello World[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "Hello World");
}

/// AcceleoEngine_ExprWithLiteral.
#[test]
fn acceleo_engine_expr_with_literal() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)]Value: ['test'/][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "Value: test");
}

/// AcceleoEngine_Let.
#[test]
fn acceleo_engine_let() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)][let x = 'hello'][x/][/let][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "hello");
}

/// AcceleoEngine_IfElse.
#[test]
fn acceleo_engine_if_else() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)][if (true)]YES[else]NO[/if][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "YES");
}

/// AcceleoEngine_ServiceCall.
#[test]
fn acceleo_engine_service_call() {
    let src = r#"
[module t(c : Class)]
[template public f(c : Class)][upper('hello')/][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    svc.register_service(
        "upper",
        Box::new(|args: &[Val], _ctx: &EvalContext| -> Val {
            match args.first() {
                Some(Val::String(s)) => Val::String(s.to_uppercase()),
                _ => Val::String(String::new()),
            }
        }),
    );
    assert_eq!(svc.evaluate_template(src, "f", &[]).unwrap(), "HELLO");
}

/// AcceleoEngine_EObjectNavigation.
#[test]
fn acceleo_engine_eobject_navigation() {
    let model = named_model("MyClass");
    let src = r#"
[module gen(c : EClass)]
[template public gen(c : EClass)]class [c.name/] {}[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(src, "gen", &[Val::Object(node_to_object(&model))])
        .unwrap();
    assert_eq!(out, "class MyClass {}");
}

/// AcceleoEngine_ForOverEList.
#[test]
fn acceleo_engine_for_over_elist() {
    let pkg = package_model("demo", &[named_model("A"), named_model("B")]);
    let src = r#"
[module gen(p : EPackage)]
[template public gen(p : EPackage)][for (c | p.eClassifiers)][c.name/] [/for][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(src, "gen", &[Val::Object(node_to_object(&pkg))])
        .unwrap();
    assert_eq!(out, "A B ");
}

/// AcceleoEngine_FileBlock.
#[test]
fn acceleo_engine_file_block() {
    let src = r#"
[module gen(c : EClass)]
[template public gen(c : EClass)]
[file (c.name + '.txt', false)]
content for [c.name/]
[/file]
[/template]
[/module]
"#;
    let model = named_model("Foo");
    let dir = fresh_dir("file_block");
    let mut svc = AcceleoService::new();
    svc.do_generate(src, Some(node_to_object(&model)), &dir)
        .expect("generate ok");
    let path = dir.join("Foo.txt");
    assert!(path.exists(), "Foo.txt should exist");
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "content for Foo\n");
}

/// AcceleoLambda_Collect.
#[test]
fn acceleo_lambda_collect() {
    let pkg = package_model("demo", &[named_model("A"), named_model("B")]);
    let src = r#"
[module gen(p : EPackage)]
[template public f(p : EPackage)]
[p.eClassifiers->collect(c | c.name)/]
[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(src, "f", &[Val::Object(node_to_object(&pkg))])
        .unwrap();
    assert!(out.contains('A'), "{out}");
    assert!(out.contains('B'), "{out}");
}

/// AcceleoLambda_SelectForAllExists.
#[test]
fn acceleo_lambda_select_for_all_exists() {
    let pkg = package_model(
        "demo",
        &[named_model("AAA"), named_model("BB"), named_model("C")],
    );
    let model = Val::Object(node_to_object(&pkg));

    let trim = |s: String| s.trim_end().to_string();

    let size_src = r#"
[module gen(p : EPackage)]
[template public f(p : EPackage)]
[p.eClassifiers->size()/]
[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(size_src, "f", std::slice::from_ref(&model))
        .unwrap();
    assert_eq!(trim(out), "3");

    let forall_src = r#"
[module gen(p : EPackage)]
[template public f(p : EPackage)]
[p.eClassifiers->forAll(c | c.name)/]
[/template]
[/module]
"#;
    let out = svc
        .evaluate_template(forall_src, "f", std::slice::from_ref(&model))
        .unwrap();
    assert_eq!(trim(out), "true");

    let exists_src = r#"
[module gen(p : EPackage)]
[template public f(p : EPackage)]
[p.eClassifiers->exists(c | c.name = 'BB')/]
[/template]
[/module]
"#;
    let out = svc
        .evaluate_template(exists_src, "f", std::slice::from_ref(&model))
        .unwrap();
    assert_eq!(trim(out), "true");

    let reject_src = r#"
[module gen(p : EPackage)]
[template public f(p : EPackage)]
[p.eClassifiers->reject(c | c.name = 'C')->size()/]
[/template]
[/module]
"#;
    let out = svc
        .evaluate_template(reject_src, "f", std::slice::from_ref(&model))
        .unwrap();
    assert_eq!(trim(out), "2");
}

/// AcceleoQuery_Eval.
#[test]
fn acceleo_query_eval() {
    let model = named_model("Foo");
    let src = r#"
[module gen(c : Class)]
[query public greet(name : String) : String = 'Hello ' + name /]
[template public f(c : Class)]
[greet(c.name)/]
[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(src, "f", &[Val::Object(node_to_object(&model))])
        .unwrap();
    assert_eq!(out.trim_end(), "Hello Foo");
}

/// AcceleoProtected_Merge.
#[test]
fn acceleo_protected_merge() {
    let model = named_model("Foo");
    let src = r#"
[module gen(c : Class)]
[template public f(c : Class)]
[file (c.name + ".txt", false)]
class [c.name/] {
[protected (body)]
// generated body
[/protected]
}
[/file]
[/template]
[/module]
"#;
    let dir = fresh_dir("protected_merge");
    let mut svc = AcceleoService::new();
    svc.do_generate(src, Some(node_to_object(&model)), &dir)
        .expect("first generate");
    let path = dir.join("Foo.txt");
    let first = std::fs::read_to_string(&path).unwrap();
    assert!(
        first.contains("BEGIN Begin Protected Region ID[body]"),
        "{first}"
    );
    assert!(
        first.contains("END End Protected Region ID[body]"),
        "{first}"
    );
    assert!(first.contains("// generated body"), "{first}");

    // Simulate a user hand-edit inside the protected region.
    let user_modified = first.replace("// generated body", "// USER HAND-EDITED CONTENT");
    std::fs::write(&path, &user_modified).unwrap();

    svc.do_generate(src, Some(node_to_object(&model)), &dir)
        .expect("second generate");
    let regenerated = std::fs::read_to_string(&path).unwrap();
    assert!(
        regenerated.contains("// USER HAND-EDITED CONTENT"),
        "{regenerated}"
    );
    assert!(!regenerated.contains("// generated body"), "{regenerated}");
}

/// AcceleoModule_Extends.
#[test]
fn acceleo_module_extends() {
    let model = named_model("Foo");

    let parent_src = r#"
[module parent(c : Class)]
[template public bracket(name : String)]
['[' + name + ']'/]
[/template]
[/module]
"#;
    let parent = emf_acceleo::parse(parent_src).expect("parse parent");

    let child_src = r#"
[module child(c : Class) extends parent]
[template public f(c : Class)]
[bracket(c.name)/]
[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    svc.engine_mut().register_module(parent);
    let out = svc
        .evaluate_template(child_src, "f", &[Val::Object(node_to_object(&model))])
        .unwrap();
    assert_eq!(out.trim_end(), "[Foo]");
}

/// AcceleoModule_ImportAndExtendsQuery.
#[test]
fn acceleo_module_import_and_extends_query() {
    let model = named_model("Bar");

    let parent_src = r#"
[module parent(c : Class)]
[query public prefix(name : String) : String = 'pre_' + name /]
[/module]
"#;
    let parent = emf_acceleo::parse(parent_src).expect("parse parent");

    let child_src = r#"
[module child(c : Class) extends parent]
[template public f(c : Class)]
[prefix(c.name)/]
[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    svc.engine_mut().register_module(parent);
    let out = svc
        .evaluate_template(child_src, "f", &[Val::Object(node_to_object(&model))])
        .unwrap();
    assert_eq!(out.trim_end(), "pre_Bar");
}

// ===== AlignmentTests.cpp =====

/// A package derived from `.xcore` source plus its per-class metadata.
struct Built {
    package: DynNode,
    classes: Vec<(String, EClass)>,
    class_objects: Vec<DynNode>,
}

impl Built {
    fn class_meta(&self, name: &str) -> &EClass {
        &self
            .classes
            .iter()
            .find(|(n, _)| n == name)
            .expect("class metadata present")
            .1
    }

    fn class_obj(&self, name: &str) -> &DynNode {
        let idx = self
            .classes
            .iter()
            .position(|(n, _)| n == name)
            .expect("class present");
        &self.class_objects[idx]
    }
}

/// Xcore primitive type name → Ecore `EDataType` name (C++ `resolveClassifier`).
fn ecore_type_name(t: &str) -> String {
    match t {
        "String" | "string" => "EString",
        "int" | "Int" | "Integer" | "integer" => "EInt",
        "boolean" | "Boolean" => "EBoolean",
        "long" | "Long" => "ELong",
        "double" | "Double" => "EDouble",
        "float" | "Float" => "EFloat",
        "short" | "Short" => "EShort",
        "char" | "Char" => "EChar",
        "byte" | "Byte" => "EByte",
        other => other,
    }
    .to_string()
}

/// Derive a runtime `EPackage` graph from `.xcore` source: parse it with
/// `emf-xcore`, then build the `EPackage`/`EClass`/`EAttribute` objects the
/// C++ `XcoreGenerator` would produce.
fn build_from_xcore(src: &str) -> Built {
    let file = emf_xcore::parse(src).expect("xcore parse ok");
    let pkg = file.package.expect("package present");

    let mut classes: Vec<(String, EClass)> = Vec::new();
    let mut class_objects: Vec<DynNode> = Vec::new();

    for c in &pkg.classes {
        // Metadata descriptor (used to instantiate model objects).
        let mut meta = EClass::new(c.name.clone(), EClassKind::Class);
        for f in &c.features {
            let mut sf = if f.kind == emf_xcore::dsl::FeatureKind::Attribute {
                EStructuralFeature::attribute(f.name.clone())
            } else {
                EStructuralFeature::reference_many(f.name.clone())
            };
            sf.set_type_name(ecore_type_name(&f.ty.type_name));
            if f.ty.multiplicity.is_many() {
                sf.set_upper_bound(-1);
            }
            meta.add_feature(sf);
        }

        // Reflective object graph (used to navigate the metamodel in templates).
        let feature_objects: Vec<DynNode> = c
            .features
            .iter()
            .map(|f| eattribute_model(&f.name, &ecore_type_name(&f.ty.type_name)))
            .collect();
        let class_obj = eclass_model(&c.name, &feature_objects);

        classes.push((c.name.clone(), meta));
        class_objects.push(class_obj);
    }

    let package = package_model(&pkg.name, &class_objects);
    Built {
        package,
        classes,
        class_objects,
    }
}

/// Alignment_XcoreDerivesEPackage_LikeJava.
#[test]
fn alignment_xcore_derives_epackage() {
    let src = r#"
        package demo {
            class Foo {
                String name
                int count
            }
            class Bar {
                boolean active
            }
        }
    "#;
    let built = build_from_xcore(src);
    assert_eq!(
        built.package.borrow().e_get("name"),
        Some(Val::String("demo".into()))
    );

    let classifiers = match built.package.borrow().e_get("eClassifiers") {
        Some(Val::List(l)) => l,
        other => panic!("expected classifier list, got {other:?}"),
    };
    assert_eq!(classifiers.len(), 2);

    let names: Vec<String> = classifiers
        .iter()
        .filter_map(|v| v.as_object())
        .filter_map(|o| match o.borrow().e_get("name") {
            Some(Val::String(s)) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Foo".to_string(), "Bar".to_string()]);

    // Reflection on the EClass object's `name` feature (C++ `fooCls->eGet`).
    let foo = built.class_obj("Foo");
    assert_eq!(foo.borrow().e_get("name"), Some(Val::String("Foo".into())));
}

/// Alignment_XcoreDerivedEClassFeatures_LikeJava.
#[test]
fn alignment_xcore_derived_eclass_features() {
    let src = r#"
        package demo {
            class Foo {
                String name
                int count
            }
        }
    "#;
    let built = build_from_xcore(src);
    let foo = built.class_obj("Foo");

    let feats = match foo.borrow().e_get("eStructuralFeatures") {
        Some(Val::List(l)) => l,
        other => panic!("expected feature list, got {other:?}"),
    };
    assert_eq!(feats.len(), 2);

    let first = feats[0].as_object().expect("feature object");
    assert_eq!(
        first.borrow().e_get("name"),
        Some(Val::String("name".into()))
    );
    let first_type = first
        .borrow()
        .e_get("eAttributeType")
        .and_then(|v| v.as_object().cloned())
        .expect("attribute type object");
    assert_eq!(
        first_type.borrow().e_get("name"),
        Some(Val::String("EString".into()))
    );

    let second = feats[1].as_object().expect("feature object");
    let second_type = second
        .borrow()
        .e_get("eAttributeType")
        .and_then(|v| v.as_object().cloned())
        .expect("attribute type object");
    assert_eq!(
        second_type.borrow().e_get("name"),
        Some(Val::String("EInt".into()))
    );
}

/// Alignment_DynamicEObject_eSet_eGet_LikeJava.
#[test]
fn alignment_dynamic_eobject_eset_eget() {
    let src = r#"
        package demo {
            class Foo {
                String name
                int count
            }
        }
    "#;
    let built = build_from_xcore(src);
    let foo_cls = built.class_meta("Foo").clone();

    // EFactory.create(Foo) == DynamicEObject over the Foo descriptor.
    let foo = obj_of(&foo_cls);
    foo.borrow_mut()
        .e_set_by_name("name", Val::String("hello".into()));
    assert_eq!(
        foo.borrow().e_get("name"),
        Some(Val::String("hello".into()))
    );
}

/// Alignment_AcceleoGeneratesFromXcorePackage_LikeJava.
#[test]
fn alignment_acceleo_generates_from_xcore_package() {
    let src = r#"
        package demo {
            class Foo { String name }
            class Bar { String name }
        }
    "#;
    let built = build_from_xcore(src);

    let mtl = r#"
[module gen(p : EPackage)]
[template public gen(p : EPackage)][for (c | p.eClassifiers)][c.name/] [/for][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(mtl, "gen", &[Val::Object(node_to_object(&built.package))])
        .unwrap();
    assert_eq!(out, "Foo Bar ");
}

/// Alignment_AcceleoGeneratesCppClassSkeleton_LikeJava.
#[test]
fn alignment_acceleo_generates_class_skeleton() {
    let src = r#"
        package demo {
            class Foo {
                String name
                int count
            }
        }
    "#;
    let built = build_from_xcore(src);
    let foo = built.class_obj("Foo").clone();

    let mtl = r#"
[module gen(c : EClass)]
[template public gen(c : EClass)]
class [c.name/] {
public:
[for (a | c.eStructuralFeatures)]    [a.eAttributeType.name/] [a.name/];
[/for]};
[/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(mtl, "gen", &[Val::Object(node_to_object(&foo))])
        .unwrap();
    let expected = "class Foo {\npublic:\n    EString name;\n    EInt count;\n};\n";
    assert_eq!(out, expected);
}

/// Alignment_AcceleoWritesFile_FromXcorePackage_LikeJava.
#[test]
fn alignment_acceleo_writes_file_from_xcore_package() {
    let src = r#"
        package demo {
            class Foo { String name }
        }
    "#;
    let built = build_from_xcore(src);
    let foo = built.class_obj("Foo").clone();

    let mtl = r#"
[module gen(c : EClass)]
[template public gen(c : EClass)]
[file (c.name + '.hpp', false)]
class [c.name/] {};
[/file]
[/template]
[/module]
"#;
    let dir = fresh_dir("align_write");
    let mut svc = AcceleoService::new();
    svc.do_generate(mtl, Some(node_to_object(&foo)), &dir)
        .expect("generate ok");
    let path = dir.join("Foo.hpp");
    assert!(path.exists(), "Foo.hpp should exist");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "class Foo {};\n");
}

/// Alignment_XcoreResource_LoadsAndDerivesEPackage_LikeJava.
///
/// C++ drives this through `XcoreResource`/`XcoreStandaloneSetup`; the Rust
/// `emf-xcore` crate ports only the parser, so the same source is parsed
/// directly and the derived package inspected (see the module header).
#[test]
fn alignment_xcore_resource_loads_and_derives_epackage() {
    let src = r#"
        package demo {
            class Foo { String name }
        }
    "#;
    let built = build_from_xcore(src);
    assert_eq!(
        built.package.borrow().e_get("name"),
        Some(Val::String("demo".into()))
    );
    let classifiers = match built.package.borrow().e_get("eClassifiers") {
        Some(Val::List(l)) => l,
        other => panic!("expected classifier list, got {other:?}"),
    };
    assert_eq!(classifiers.len(), 1);
    let first = classifiers[0].as_object().expect("classifier object");
    assert_eq!(
        first.borrow().e_get("name"),
        Some(Val::String("Foo".into()))
    );
}

/// Alignment_AcceleoComplexTemplate_IfForNested_LikeJava.
#[test]
fn alignment_acceleo_complex_template_if_for_nested() {
    let src = r#"
        package demo {
            class Foo {
                String name
                int count
            }
            class Bar {
                boolean active
            }
        }
    "#;
    let built = build_from_xcore(src);

    let mtl = r#"
[module gen(p : EPackage)]
[template public gen(p : EPackage)][for (c | p.eClassifiers)][if (c.eStructuralFeatures->size() = 1)][c.name/]: single
[else][c.name/]: multi: [c.eStructuralFeatures->size()/]
[/if][/for][/template]
[/module]
"#;
    let mut svc = AcceleoService::new();
    let out = svc
        .evaluate_template(mtl, "gen", &[Val::Object(node_to_object(&built.package))])
        .unwrap();
    assert_eq!(out, "Foo: multi: 2\nBar: single\n");
}

// Keep the `ObjectRef` import meaningful even if a future refactor drops a use.
#[allow(dead_code)]
fn _assert_object_ref(_: ObjectRef) {}
