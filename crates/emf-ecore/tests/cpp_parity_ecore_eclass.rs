//! C++ parity suite: emf-ecore EClass reflection.
//!
//! Ports `EClassImplTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `EClass`.
//!
//! Rust resolves inheritance by *name* through a `PackageRegistry` (C++ uses
//! direct super-type pointers). Behavior notes tracked in PARITY_TRACKER:
//!   - Rust `is_super_type_of` is strict (never self); C++ is reflexive.
//!   - Rust `EClass` has no inherited-operation aggregation (`eAllOperations`).
//!   - `eAllContainments` derives from `e_all_references` + `is_containment`.
use emf_ecore::{
    make_package_ref, EClass, EClassKind, EOperation, EPackage, EStructuralFeature, PackageRegistry,
};

/// Registry containing a `Store` class with attributes name(0)/address(1)/id(2,
/// ID), and a `Library` subclass adding reference `books` (3).
fn make_model() -> (EClass, EClass, PackageRegistry) {
    let mut store = EClass::new("Store", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_feature_id(0);
    let mut address = EStructuralFeature::attribute("address");
    address.set_feature_id(1);
    let mut id = EStructuralFeature::attribute("id");
    id.set_feature_id(2);
    store.add_feature(name);
    store.add_feature(address);
    store.add_feature(id);
    store.set_id_feature(2);

    let mut lib = EClass::new("Library", EClassKind::Class);
    let _ = lib.add_super_type("Store");
    let mut books = EStructuralFeature::reference_many("books");
    books.set_containment(true);
    books.set_feature_id(3);
    lib.add_feature(books);

    let mut pkg = EPackage::new("pkg");
    pkg.add_class(store.clone());
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));

    (store, lib, reg)
}

#[test]
fn create_eclass_and_attribute() {
    let mut cls = EClass::new("Person", EClassKind::Class);
    let mut id = EStructuralFeature::attribute("id");
    let mut age = EStructuralFeature::attribute("age");
    cls.add_feature(id);
    cls.add_feature(age);

    assert_eq!(cls.name(), "Person");
    assert_eq!(cls.e_structural_features().len(), 2);
    let names: Vec<String> = cls
        .e_structural_features()
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    assert!(names.contains(&"id".to_string()));
    assert!(names.contains(&"age".to_string()));
}

#[test]
fn feature_id_lookup() {
    let mut cls = EClass::new("Book", EClassKind::Class);
    let mut title = EStructuralFeature::attribute("title");
    title.set_feature_id(0);
    let mut isbn = EStructuralFeature::attribute("isbn");
    isbn.set_feature_id(1);
    cls.add_feature(title);
    cls.add_feature(isbn);
    let title = cls
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "title")
        .unwrap();
    let isbn = cls
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "isbn")
        .unwrap();
    assert_eq!(title.feature_id(), 0);
    assert_eq!(isbn.feature_id(), 1);
}

#[test]
fn abstract_and_interface() {
    let mut cls = EClass::new("A", EClassKind::Class);
    assert!(!cls.is_abstract());
    cls.set_abstract(true);
    assert!(cls.is_abstract());
    cls.set_abstract(false);
    assert!(!cls.is_abstract());

    let iface = EClass::new("I", EClassKind::Interface);
    assert!(iface.is_interface());
}

#[test]
fn get_structural_feature_by_id() {
    let mut cls = EClass::new("Cls", EClassKind::Class);
    let mut a1 = EStructuralFeature::attribute("a1");
    a1.set_feature_id(7);
    cls.add_feature(a1);
    let a1 = &cls.e_structural_features()[0];
    assert_eq!(a1.feature_id(), 7);
    assert!(cls
        .e_structural_features()
        .iter()
        .find(|f| f.feature_id() == 99)
        .is_none());
}

#[test]
fn is_super_type_of_relationship() {
    let (store, lib, reg) = make_model();
    assert!(lib.is_super_type_of("Store", &reg));
    assert!(!store.is_super_type_of("Library", &reg));
}

#[test]
fn e_all_super_types_transitive_closure() {
    let (store, lib, reg) = make_model();
    assert_eq!(store.e_all_super_types(&reg).len(), 0);
    assert_eq!(lib.e_all_super_types(&reg), vec!["Store".to_string()]);
}

#[test]
fn e_all_super_types_multilevel() {
    let mut a = EClass::new("A", EClassKind::Class);
    let mut b = EClass::new("B", EClassKind::Class);
    let _ = b.add_super_type("A");
    let mut c = EClass::new("C", EClassKind::Class);
    let _ = c.add_super_type("B");
    let mut pkg = EPackage::new("pkg");
    pkg.add_class(a);
    pkg.add_class(b);
    pkg.add_class(c);
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    let c = reg.find_class("C").expect("C registered");
    assert_eq!(
        c.e_all_super_types(&reg),
        vec!["A".to_string(), "B".to_string()]
    );
}

#[test]
fn e_all_attributes_inherits_from_parent() {
    let (store, lib, reg) = make_model();
    assert_eq!(store.e_all_attributes(&reg).len(), 3);
    assert_eq!(lib.e_all_attributes(&reg).len(), 3);
    let names: Vec<String> = lib
        .e_all_attributes(&reg)
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    assert!(names.contains(&"name".to_string()));
    assert!(names.contains(&"address".to_string()));
    assert!(names.contains(&"id".to_string()));
}

#[test]
fn e_all_references_includes_inherited() {
    let (store, lib, reg) = make_model();
    assert_eq!(store.e_all_references(&reg).len(), 0);
    let refs = lib.e_all_references(&reg);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name(), "books");
    assert!(refs[0].is_containment()); // eAllContainments subset
}

#[test]
fn e_all_structural_features_and_feature_count() {
    let (store, lib, reg) = make_model();
    assert_eq!(store.e_all_structural_features(&reg).len(), 3);
    assert_eq!(lib.e_all_structural_features(&reg).len(), 4);
}

#[test]
fn e_id_attribute_finds_id_marked() {
    let (store, _lib, reg) = make_model();
    assert_eq!(store.id_feature(), Some(2));
    let names: Vec<String> = store
        .e_all_attributes(&reg)
        .iter()
        .filter(|f| f.feature_id() == store.id_feature().unwrap())
        .map(|f| f.name().to_string())
        .collect();
    assert_eq!(names, vec!["id".to_string()]);
    // Rust `id_feature` is own-class-only. The (inherited) ID attribute is still
    // discoverable through `e_all_attributes`; the C++ cross-hierarchy
    // `getEIDAttribute` lookup has no direct Rust counterpart yet (gap).
    let ids: Vec<String> = _lib
        .e_all_attributes(&reg)
        .iter()
        .filter(|f| f.feature_id() == 2)
        .map(|f| f.name().to_string())
        .collect();
    assert_eq!(ids, vec!["id".to_string()]);
}

#[test]
fn e_operations_own_metadata() {
    let (_store, mut lib, _reg) = make_model();
    let mut close = EOperation::new("close");
    close.set_operation_id(1);
    lib.add_operation(close);
    let names: Vec<String> = lib
        .e_operations()
        .iter()
        .map(|o| o.name().to_string())
        .collect();
    assert_eq!(names, vec!["close".to_string()]);
}
