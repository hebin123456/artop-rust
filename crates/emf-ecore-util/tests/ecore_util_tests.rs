//! Rust port parity tests for `EcoreUtilTests.cpp` and
//! `EcoreUtilExtendedTests.cpp` (C++ `emf-ecore-util`, aligned to Java
//! `org.eclipse.emf.ecore.util.EcoreUtil`).

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::value::{ObjectRef, Val};
use emf_ecore::{
    adopt_single, make_package_ref, node_to_object, DynNode, DynamicEObject, EClass, EClassKind,
    EDataType, EPackage, EStructuralFeature, PackageRegistry,
};
use emf_ecore_util::equality_helper::EqualityHelper;
use emf_ecore_util::ecore_util::{
    convert_to_string, copy, copy_all, create_from_string, e_all_contents, equals, equals_value,
    get_e_classifier, get_id, get_uri, is_ancestor_of, remove, resolve, resolve_all, set_id,
};

// ---------- helpers ----------

fn registry_with(pkg: EPackage) -> PackageRegistry {
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

/// Person EClass: `name` (EString, id) + `age` (EInt).
fn person_class() -> EClass {
    let mut cls = EClass::new("Person", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    name.set_id(true);
    let mut age = EStructuralFeature::attribute("age");
    age.set_type_name("EInt");
    cls.add_feature(name);
    cls.add_feature(age);
    cls
}

/// A registry whose package owns exactly one `Person` class.
fn person_registry() -> PackageRegistry {
    let mut pkg = EPackage::new("people");
    pkg.set_ns_prefix("ppl");
    pkg.add_class(person_class());
    registry_with(pkg)
}

fn make_obj(cls: EClass, reg: PackageRegistry) -> ObjectRef {
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg)))
}

fn set(obj: &ObjectRef, name: &str, val: Val) {
    obj.borrow_mut().e_set(name, val);
}

fn set_str(obj: &ObjectRef, name: &str, s: &str) {
    set(obj, name, Val::String(s.into()));
}

fn str_of(obj: &ObjectRef, name: &str) -> String {
    obj.borrow()
        .e_get(name)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Tree model: `Parent { name; child: Child (containment, single) }`,
/// `Child { cname }`.
fn tree_registry() -> PackageRegistry {
    let mut cls_parent = EClass::new("Parent", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    cls_parent.add_feature(name);
    let mut child_ref = EStructuralFeature::reference("child");
    child_ref.set_type_name("Child");
    child_ref.set_containment(true);
    cls_parent.add_feature(child_ref);

    let mut cls_child = EClass::new("Child", EClassKind::Class);
    let mut cname = EStructuralFeature::attribute("cname");
    cname.set_type_name("EString");
    cls_child.add_feature(cname);

    let mut pkg = EPackage::new("tree");
    pkg.set_ns_prefix("tr");
    pkg.add_class(cls_parent);
    pkg.add_class(cls_child);
    registry_with(pkg)
}

// ---------- equals / equalsValue (EcoreUtilTests.cpp) ----------

#[test]
fn equals_same_object_true() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    assert!(equals(Some(&obj), Some(&obj)));
}

#[test]
fn equals_both_null_true() {
    assert!(equals(None, None));
}

#[test]
fn equals_one_null_false() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    assert!(!equals(Some(&obj), None));
    assert!(!equals(None, Some(&obj)));
}

#[test]
fn equals_different_classes_false() {
    // Two independently constructed Person classes -> distinct class identity.
    let reg_a = person_registry();
    let reg_b = person_registry();
    let a = make_obj(person_class(), reg_a);
    let b = make_obj(person_class(), reg_b);
    assert!(!equals(Some(&a), Some(&b)));
}

#[test]
fn equals_same_class_same_values_true() {
    let reg = person_registry();
    let cls = person_class();
    let a = make_obj(cls.clone(), reg.clone());
    let b = make_obj(cls, reg);
    set_str(&a, "name", "alice");
    set_str(&b, "name", "alice");
    assert!(equals(Some(&a), Some(&b)));
}

#[test]
fn equals_same_class_different_values_false() {
    let reg = person_registry();
    let cls = person_class();
    let a = make_obj(cls.clone(), reg.clone());
    let b = make_obj(cls, reg);
    set_str(&a, "name", "alice");
    set_str(&b, "name", "bob");
    assert!(!equals(Some(&a), Some(&b)));
}

#[test]
fn equals_value_string() {
    assert!(equals_value(Some(&Val::string("x")), Some(&Val::string("x"))));
    assert!(!equals_value(Some(&Val::string("x")), Some(&Val::string("y"))));
}

#[test]
fn equals_value_int() {
    assert!(equals_value(Some(&Val::Int(42)), Some(&Val::Int(42))));
    assert!(!equals_value(Some(&Val::Int(42)), Some(&Val::Int(43))));
}

#[test]
fn equals_value_both_empty_true() {
    assert!(equals_value(None, None));
}

#[test]
fn equals_value_one_empty_false() {
    assert!(!equals_value(None, Some(&Val::Int(42))));
    assert!(!equals_value(Some(&Val::Int(42)), None));
}

// ---------- getID / setID ----------

#[test]
fn get_id_no_id_set_empty() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    assert_eq!(get_id(&obj), "");
}

#[test]
fn set_id_then_get_id() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    assert!(set_id(&obj, "alice-001"));
    assert_eq!(get_id(&obj), "alice-001");
}

#[test]
fn set_id_overwrites() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    set_id(&obj, "first");
    set_id(&obj, "second");
    assert_eq!(get_id(&obj), "second");
}

#[test]
fn get_id_no_id_attribute_empty() {
    // A class with only a non-ID attribute -> ID reads as "".
    let mut pkg = EPackage::new("noid");
    let mut cls = EClass::new("NoId", EClassKind::Class);
    let mut x = EStructuralFeature::attribute("x");
    x.set_type_name("EString");
    cls.add_feature(x);
    pkg.add_class(cls);
    let reg = registry_with(pkg);
    let obj = make_obj(reg.find_class("NoId").unwrap(), reg);
    set_str(&obj, "x", "v");
    assert_eq!(get_id(&obj), "");
}

// ---------- getURI ----------

#[test]
fn get_uri_root_object_no_resource_contains_urn() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    let uri = get_uri(&obj);
    assert!(uri.contains("urn:emf"), "got {uri}");
}

// ---------- isAncestor (EClass, EObject) ----------

#[test]
fn is_ancestor_same_class_true() {
    let reg = person_registry();
    let cls = person_class();
    let obj = make_obj(cls.clone(), reg);
    assert!(is_ancestor_of(&cls, &obj));
}

#[test]
fn is_ancestor_super_type_true() {
    let mut pkg = EPackage::new("hier");
    let mut parent = EClass::new("Parent", EClassKind::Class);
    let mut child = EClass::new("Child", EClassKind::Class);
    child.add_super_type("Parent").unwrap();
    pkg.add_class(parent);
    pkg.add_class(child);
    let reg = registry_with(pkg);
    let parent_cls = reg.find_class("Parent").unwrap();
    let child_cls = reg.find_class("Child").unwrap();
    let obj = make_obj(child_cls, reg);
    assert!(is_ancestor_of(&parent_cls, &obj));
}

#[test]
fn is_ancestor_unrelated_false() {
    let mut pkg = EPackage::new("hier2");
    let parent = EClass::new("Parent", EClassKind::Class);
    pkg.add_class(parent);
    pkg.add_class(person_class());
    let reg = registry_with(pkg);
    let parent_cls = reg.find_class("Parent").unwrap();
    let person_cls = reg.find_class("Person").unwrap();
    let obj = make_obj(person_cls, reg);
    assert!(!is_ancestor_of(&parent_cls, &obj));
}

// ---------- getEClassifier ----------

#[test]
fn get_e_classifier_found() {
    let mut pkg = EPackage::new("P");
    pkg.add_data_type(EDataType::new("MyType", "java.lang.String"));
    let found = get_e_classifier(&pkg, "MyType");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name(), "MyType");
}

#[test]
fn get_e_classifier_not_found_null() {
    let pkg = EPackage::new("P");
    assert!(get_e_classifier(&pkg, "Missing").is_none());
}

#[test]
fn get_e_classifier_non_data_type_null() {
    let mut pkg = EPackage::new("P");
    pkg.add_class(EClass::new("C", EClassKind::Class));
    assert!(get_e_classifier(&pkg, "C").is_none());
}

// ---------- createFromString / convertToString ----------

#[test]
fn create_from_string_e_string() {
    assert_eq!(
        create_from_string("EString", "hello"),
        Val::String("hello".into())
    );
}

#[test]
fn create_from_string_e_int() {
    assert_eq!(create_from_string("EInt", "123"), Val::Int(123));
}

#[test]
fn create_from_string_e_boolean() {
    assert_eq!(create_from_string("EBoolean", "true"), Val::Bool(true));
    assert_eq!(create_from_string("EBoolean", "false"), Val::Bool(false));
}

#[test]
fn convert_to_string_e_string() {
    assert_eq!(
        convert_to_string("EString", &Val::String("hi".into())),
        "hi"
    );
}

#[test]
fn convert_to_string_e_int() {
    assert_eq!(convert_to_string("EInt", &Val::Int(42)), "42");
}

#[test]
fn convert_to_string_e_boolean() {
    assert_eq!(convert_to_string("EBoolean", &Val::Bool(true)), "true");
    assert_eq!(convert_to_string("EBoolean", &Val::Bool(false)), "false");
}

#[test]
fn create_convert_round_trip_e_int() {
    let v = create_from_string("EInt", "99");
    assert_eq!(convert_to_string("EInt", &v), "99");
}

// ---------- getAllContents (EcoreUtilExtendedTests.cpp) ----------

#[test]
fn get_all_contents_no_children_empty() {
    let reg = tree_registry();
    let parent = make_obj(reg.find_class("Parent").unwrap(), reg);
    assert!(e_all_contents(&parent).is_empty());
}

#[test]
fn get_all_contents_single_child() {
    let reg = tree_registry();
    let parent_cls = reg.find_class("Parent").unwrap();
    let child_cls = reg.find_class("Child").unwrap();
    let parent: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(parent_cls.clone(), reg.clone())));
    let child: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(child_cls, reg)));
    adopt_single(&parent, "child", &child);
    let p_obj = node_to_object(&parent);
    let all = e_all_contents(&p_obj);
    assert_eq!(all.len(), 1);
    assert!(Rc::ptr_eq(&all[0], &node_to_object(&child)));
}

#[test]
fn get_all_contents_nested_depth_first() {
    // parent -> child1 (a Parent) -> grandchild (a Child)
    let reg = tree_registry();
    let parent_cls = reg.find_class("Parent").unwrap();
    let child_cls = reg.find_class("Child").unwrap();
    let parent: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(parent_cls.clone(), reg.clone())));
    let child1: DynNode =
        Rc::new(RefCell::new(DynamicEObject::new_in(parent_cls, reg.clone())));
    let grandchild: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(child_cls, reg)));
    adopt_single(&parent, "child", &child1);
    adopt_single(&child1, "child", &grandchild);

    let all = e_all_contents(&node_to_object(&parent));
    assert_eq!(all.len(), 2, "depth-first: child1 then grandchild");
    assert!(Rc::ptr_eq(&all[0], &node_to_object(&child1)));
    assert!(Rc::ptr_eq(&all[1], &node_to_object(&grandchild)));
}

// ---------- remove ----------

#[test]
fn remove_single_valued_containment() {
    let reg = tree_registry();
    let parent_cls = reg.find_class("Parent").unwrap();
    let child_cls = reg.find_class("Child").unwrap();
    let parent: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(parent_cls, reg.clone())));
    let child: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(child_cls, reg)));
    adopt_single(&parent, "child", &child);
    let c_obj = node_to_object(&child);
    assert!(remove(&c_obj));
    let p_obj = node_to_object(&parent);
    let remaining = p_obj.borrow().e_get("child");
    assert!(
        remaining.map(|v| v.is_null()).unwrap_or(true),
        "parent.child == null after remove"
    );
}

#[test]
fn remove_root_object_no_container_no_crash() {
    let reg = tree_registry();
    let parent = make_obj(reg.find_class("Parent").unwrap(), reg);
    let _ = remove(&parent);
}

// ---------- copy / copyAll ----------

#[test]
fn copy_preserves_attribute_values() {
    let reg = tree_registry();
    let parent = make_obj(reg.find_class("Parent").unwrap(), reg);
    set_str(&parent, "name", "root");
    let cp = copy(&parent).expect("copy succeeds");
    assert!(!Rc::ptr_eq(&parent, &cp));
    assert_eq!(cp.borrow().e_class(), "Parent");
    assert_eq!(str_of(&cp, "name"), "root");
}

#[test]
fn copy_deep_copy_containment() {
    let reg = tree_registry();
    let parent_cls = reg.find_class("Parent").unwrap();
    let child_cls = reg.find_class("Child").unwrap();
    let parent: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(parent_cls, reg.clone())));
    let child: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(child_cls, reg)));
    set(&(node_to_object(&child)), "cname", Val::String("c1".into()));
    adopt_single(&parent, "child", &child);
    let c_obj = node_to_object(&child);
    let p_obj = node_to_object(&parent);

    let cp = copy(&p_obj).expect("copy succeeds");
    assert!(!Rc::ptr_eq(&cp, &p_obj));
    let cp_child = cp.borrow().e_get("child").and_then(|v| v.as_object().cloned());
    let cp_child = cp_child.expect("copied parent has child");
    assert!(!Rc::ptr_eq(&cp_child, &c_obj), "child copy is a new object");
    assert_eq!(str_of(&cp_child, "cname"), "c1");

    // The child copy's container points at the parent copy.
    let container = cp_child.borrow().e_container();
    assert!(
        container.map(|p| Rc::ptr_eq(&p, &cp)).unwrap_or(false),
        "copied child eContainer == copied parent"
    );
}

#[test]
fn copy_distinct_instance() {
    let reg = tree_registry();
    let parent = make_obj(reg.find_class("Parent").unwrap(), reg);
    let cp = copy(&parent).unwrap();
    assert!(!Rc::ptr_eq(&parent, &cp));
}

#[test]
fn copy_all_empty_input_empty() {
    let out = copy_all(&[]);
    assert!(out.is_empty());
}

#[test]
fn copy_all_multiple_objects() {
    let reg = tree_registry();
    let a = make_obj(reg.find_class("Parent").unwrap(), reg.clone());
    set_str(&a, "name", "a");
    let b = make_obj(reg.find_class("Parent").unwrap(), reg);
    set_str(&b, "name", "b");
    let out = copy_all(&[a.clone(), b.clone()]);
    assert_eq!(out.len(), 2);
    assert!(!Rc::ptr_eq(&out[0], &a));
    assert!(!Rc::ptr_eq(&out[1], &b));
    assert_eq!(str_of(&out[0], "name"), "a");
    assert_eq!(str_of(&out[1], "name"), "b");
}

// ---------- resolve / resolveAll ----------

#[test]
fn resolve_non_proxy_returns_same() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    let r = resolve(Some(&obj)).unwrap();
    assert!(Rc::ptr_eq(&r, &obj));
}

#[test]
fn resolve_null_null() {
    assert!(resolve(None).is_none());
}

#[test]
fn resolve_all_non_proxy_no_crash() {
    let reg = tree_registry();
    let parent = make_obj(reg.find_class("Parent").unwrap(), reg);
    resolve_all(&parent);
}

// ---------- EqualityHelper ----------

#[test]
fn equality_helper_equals_same_object() {
    let reg = person_registry();
    let obj = make_obj(person_class(), reg);
    let eh = EqualityHelper::new();
    assert!(eh.equals(Some(&obj), Some(&obj)));
}

#[test]
fn equality_helper_hash_code_string_nonzero() {
    let eh = EqualityHelper::new();
    let h = eh.hash_code(Some(&Val::string("a")));
    assert_ne!(h, 0.0);
}

#[test]
fn equality_helper_hash_code_null_object_zero() {
    let eh = EqualityHelper::new();
    assert_eq!(eh.hash_code(None), 0.0);
}