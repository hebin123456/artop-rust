//! Rust port parity tests for `CopierTests.cpp` (C++ `emf-ecore-util`
//! `EcoreUtil.Copier`, aligned to Java `org.eclipse.emf.ecore.util.EcoreUtil$Copier`).

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::value::{ObjectRef, Val};
use emf_ecore::{
    adopt_single, make_package_ref, node_to_object, DynNode, DynamicEObject, EClass, EClassKind,
    EPackage, EStructuralFeature, PackageRegistry,
};
use emf_ecore_util::copier::Copier;
use emf_ecore_util::ecore_util::copy;

/// Person { name:EString(id) } + friend:Person (non-containment, single).
struct PersonModel {
    cls: EClass,
    reg: PackageRegistry,
}

impl PersonModel {
    fn new() -> Self {
        let mut cls = EClass::new("Person", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        name.set_id(true);
        let mut friend_ref = EStructuralFeature::reference("friend");
        friend_ref.set_type_name("Person");
        friend_ref.set_containment(false);
        cls.add_feature(name);
        cls.add_feature(friend_ref);
        let mut pkg = EPackage::new("Pkg");
        pkg.set_ns_prefix("pk");
        pkg.add_class(cls.clone());
        let mut reg = PackageRegistry::new();
        reg.register(make_package_ref(pkg));
        Self { cls, reg }
    }

    fn person(&self, name: &str) -> ObjectRef {
        let o: ObjectRef = Rc::new(RefCell::new(DynamicEObject::new_in(
            self.cls.clone(),
            self.reg.clone(),
        )));
        o.borrow_mut().e_set("name", Val::string(name));
        o
    }
}

fn str_of(obj: &ObjectRef, name: &str) -> String {
    obj.borrow()
        .e_get(name)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

#[test]
fn copy_returns_new_instance() {
    let m = PersonModel::new();
    let src = m.person("alice");
    let mut c = Copier::new();
    let cp = c.copy(&src).unwrap();
    assert!(!Rc::ptr_eq(&cp, &src));
    assert_eq!(cp.borrow().e_class(), "Person");
}

#[test]
fn copy_preserves_attribute() {
    let m = PersonModel::new();
    let src = m.person("alice");
    let mut c = Copier::new();
    let cp = c.copy(&src).unwrap();
    assert_eq!(str_of(&cp, "name"), "alice");
}

#[test]
fn copy_idempotent_returns_same_instance() {
    let m = PersonModel::new();
    let src = m.person("alice");
    let mut c = Copier::new();
    let cp1 = c.copy(&src).unwrap();
    let cp2 = c.copy(&src).unwrap();
    assert!(Rc::ptr_eq(&cp1, &cp2));
}

#[test]
fn get_unknown_source_none() {
    let m = PersonModel::new();
    let src = m.person("alice");
    let c = Copier::new();
    assert!(c.get(&src).is_none());
}

#[test]
fn get_after_copy_returns_copy() {
    let m = PersonModel::new();
    let src = m.person("alice");
    let mut c = Copier::new();
    let cp = c.copy(&src).unwrap();
    assert!(c.get(&src).map(|x| Rc::ptr_eq(&x, &cp)).unwrap_or(false));
}

#[test]
fn copy_all_empty_input_empty() {
    let mut c = Copier::new();
    assert!(c.copy_all(&[]).unwrap().is_empty());
}

#[test]
fn copy_all_multiple_objects() {
    let m = PersonModel::new();
    let a = m.person("a");
    let b = m.person("b");
    let mut c = Copier::new();
    let out = c.copy_all(&[a.clone(), b.clone()]).unwrap();
    assert_eq!(out.len(), 2);
    assert!(!Rc::ptr_eq(&out[0], &a));
    assert!(!Rc::ptr_eq(&out[1], &b));
    assert!(c.get(&a).map(|x| Rc::ptr_eq(&x, &out[0])).unwrap_or(false));
    assert!(c.get(&b).map(|x| Rc::ptr_eq(&x, &out[1])).unwrap_or(false));
    assert_eq!(str_of(&out[0], "name"), "a");
    assert_eq!(str_of(&out[1], "name"), "b");
}

#[test]
fn copy_references_non_containment_ref_points_to_copy() {
    let m = PersonModel::new();
    let a = m.person("a");
    let b = m.person("b");
    a.borrow_mut().e_set("friend", Val::Object(Rc::clone(&b)));

    let mut c = Copier::new();
    let a_copy = c.copy(&a).unwrap();
    let b_copy = c.copy(&b).unwrap();
    // Before copy_references, a'.friend may still point at source b.
    c.copy_references().unwrap();

    let target = a_copy
        .borrow()
        .e_get("friend")
        .and_then(|v| v.as_object().cloned())
        .expect("aCopy has friend");
    assert!(Rc::ptr_eq(&target, &b_copy), "aCopy.friend == bCopy");
    assert!(!Rc::ptr_eq(&target, &b), "not the source b");
}

#[test]
fn copy_references_not_called_ref_still_points_to_source() {
    let m = PersonModel::new();
    let a = m.person("a");
    let b = m.person("b");
    a.borrow_mut().e_set("friend", Val::Object(Rc::clone(&b)));

    let mut c = Copier::new();
    let _a_copy = c.copy(&b).unwrap();
    let a_copy = c.copy(&a).unwrap();
    // Without copy_references, aCopy.friend still points at source b.
    let target = a_copy
        .borrow()
        .e_get("friend")
        .and_then(|v| v.as_object().cloned())
        .expect("aC has friend");
    assert!(Rc::ptr_eq(&target, &b), "aCopy.friend == source b");
}

#[test]
fn copy_references_keeps_mapping() {
    let m = PersonModel::new();
    let a = m.person("alice");
    let mut c = Copier::new();
    let a_copy = c.copy(&a).unwrap();
    c.copy_references().unwrap();
    assert!(c.get(&a).map(|x| Rc::ptr_eq(&x, &a_copy)).unwrap_or(false));
}

#[test]
fn ecore_util_copy_with_containment_deep_copy() {
    let mut cls = EClass::new("Parent", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    let mut child_ref = EStructuralFeature::reference("child");
    child_ref.set_type_name("Parent");
    child_ref.set_containment(true);
    cls.add_feature(name);
    cls.add_feature(child_ref);
    let mut pkg = EPackage::new("P");
    pkg.set_ns_prefix("p");
    pkg.add_class(cls.clone());
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));

    let root: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(cls.clone(), reg.clone())));
    root.borrow_mut().e_set_by_name("name", Val::string("root"));
    let kid: DynNode = Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg)));
    kid.borrow_mut().e_set_by_name("name", Val::string("kid"));
    adopt_single(&root, "child", &kid);
    let root_obj = node_to_object(&root);
    let kid_obj = node_to_object(&kid);

    let cp = copy(&root_obj).expect("EcoreUtil::copy");
    let cp_kid = cp
        .borrow()
        .e_get("child")
        .and_then(|v| v.as_object().cloned())
        .expect("copy has child");
    assert!(!Rc::ptr_eq(&cp_kid, &kid_obj));
    assert_eq!(str_of(&cp_kid, "name"), "kid");
    let container = cp_kid.borrow().e_container();
    assert!(
        container.map(|p| Rc::ptr_eq(&p, &cp)).unwrap_or(false),
        "copied child eContainer == copied parent"
    );
}

#[test]
fn copy_references_unknown_target_keeps_original() {
    let m = PersonModel::new();
    let a = m.person("a");
    let c = m.person("c");
    a.borrow_mut().e_set("friend", Val::Object(Rc::clone(&c)));

    let mut copier = Copier::new();
    let a_copy = copier.copy(&a).unwrap(); // only copy a, not c
    copier.copy_references().unwrap();
    let target = a_copy
        .borrow()
        .e_get("friend")
        .and_then(|v| v.as_object().cloned())
        .expect("aC has friend");
    // c was never copied -> aC.friend stays pointing at source c.
    assert!(Rc::ptr_eq(&target, &c), "aC.friend == source c");
}