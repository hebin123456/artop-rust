//! Integration tests porting the C++ `emf-ecore` reflection unit tests
//! (EClassImplTests / ETypedElementImplTests / EPackageImplTests /
//! EcorePackageTests / DataTypeUtilTests) to the Rust API.
//!
//! In this crate inheritance is *metadata*: `EClass::e_super_types` holds parent
//! class *names* and a `PackageRegistry` resolves the graph. `EClass` is
//! `Clone` (no stable object-pointer identity), so pointer-identity assertions
//! from the C++ tests are re-expressed by `feature_id` / name.
//!
//! Some EClass queries not exposed on the public type are composed here from
//! the public helper API (add_module-level helper functions only, per task).

use emf_ecore::structural::FeatureKind;
use emf_ecore::*;
use std::collections::HashSet;

// ===========================================================================
// Module-level helpers (composed from public API; no `src/*` changes).
// ===========================================================================

/// A structural attribute with a type name and explicit FeatureID.
fn tagged_attr(name: &str, type_name: &str, fid: i32) -> EStructuralFeature {
    let mut f = EStructuralFeature::attribute(name);
    f.set_type_name(type_name);
    f.set_feature_id(fid);
    f
}

/// An operation with an explicit operation ID.
fn tagged_op(name: &str, oid: i32) -> EOperation {
    let mut op = EOperation::new(name);
    op.set_operation_id(oid);
    op
}

/// `eAllContainments`: the containment references among `eAllReferences`.
fn e_all_containments(cls: &EClass, reg: &PackageRegistry) -> Vec<EStructuralFeature> {
    cls.e_all_references(reg)
        .into_iter()
        .filter(|f| f.is_containment())
        .collect()
}

/// `eAllOperations`: inherited operations (ancestors-first) then own, dedup by
/// operation id (mirrors `e_all_structural_features`).
fn e_all_operations(cls: &EClass, reg: &PackageRegistry) -> Vec<EOperation> {
    let mut out: Vec<EOperation> = Vec::new();
    let mut seen: HashSet<i32> = HashSet::new();
    let mut push = |ops: &[EOperation]| {
        for op in ops {
            let id = op.operation_id();
            if id >= 0 && !seen.insert(id) {
                continue;
            }
            out.push(op.clone());
        }
    };
    for name in cls.e_all_super_types(reg) {
        if let Some(parent) = reg.find_class(&name) {
            push(parent.e_operations());
        }
    }
    push(cls.e_operations());
    out
}

/// `getOperationCount` = number of inherited + own operations.
fn get_operation_count(cls: &EClass, reg: &PackageRegistry) -> usize {
    e_all_operations(cls, reg).len()
}

/// `getEOperation(int)`: operation with a matching operation id.
fn operation_by_id(cls: &EClass, id: i32, reg: &PackageRegistry) -> Option<EOperation> {
    e_all_operations(cls, reg)
        .into_iter()
        .find(|op| op.operation_id() == id)
}

/// `getEOperation(String)`: first operation with a matching name.
fn operation_by_name(cls: &EClass, name: &str, reg: &PackageRegistry) -> Option<EOperation> {
    e_all_operations(cls, reg)
        .into_iter()
        .find(|op| op.name() == name)
}

/// `getEStructuralFeature(int)`: structural feature with a matching feature id.
fn structural_feature_by_id(
    cls: &EClass,
    fid: i32,
    reg: &PackageRegistry,
) -> Option<EStructuralFeature> {
    cls.e_all_structural_features(reg)
        .into_iter()
        .find(|f| f.feature_id() == fid)
}

/// `getFeatureCount` = inherited + own structural feature count.
fn feature_count(cls: &EClass, reg: &PackageRegistry) -> usize {
    cls.e_all_structural_features(reg).len()
}

/// `getEIDAttribute`: the ID-marked attribute among self + ancestors.
fn e_id_attribute(cls: &EClass, reg: &PackageRegistry) -> Option<EStructuralFeature> {
    let id_fid = {
        let mut f = cls.id_feature();
        for sup in cls.e_all_super_types(reg) {
            if let Some(parent) = reg.find_class(&sup) {
                if let Some(ff) = parent.id_feature() {
                    f = Some(ff);
                    break;
                }
            }
        }
        f
    }?;
    cls.e_all_structural_features(reg)
        .into_iter()
        .find(|f| f.feature_id() == id_fid)
}

/// `getOverride(EOperation)`: an operation in the hierarchy with the same name
/// but a different operation id than the base operation.
fn override_operation(
    cls: &EClass,
    base: &EOperation,
    reg: &PackageRegistry,
) -> Option<EOperation> {
    e_all_operations(cls, reg)
        .into_iter()
        .find(|op| op.name() == base.name() && op.operation_id() != base.operation_id())
}

/// Build a registry with `Store` (name/address/id + open op) and `Library`
/// (extends Store; owns books containment + close op).
fn store_library_registry() -> PackageRegistry {
    let mut pkg = EPackage::new("sample");
    pkg.set_ns_uri("http://sample/storelib");

    let mut store = EClass::new("Store", EClassKind::Class);
    store.add_feature(tagged_attr("name", "EString", 0));
    store.add_feature(tagged_attr("address", "EString", 1));
    store.add_feature(tagged_attr("id", "EString", 2));
    store.set_id_feature(2);
    store.add_operation(tagged_op("open", 0));
    pkg.add_class(store);

    let mut library = EClass::new("Library", EClassKind::Class);
    library.add_super_type("Store").unwrap();
    let mut books = EStructuralFeature::reference_many("books"); // many
    books.set_containment(true);
    books.set_type_name("Book");
    books.set_feature_id(3);
    library.add_feature(books);
    library.add_operation(tagged_op("close", 1));
    pkg.add_class(library);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

/// Build a registry with the A <- B <- C multilevel inheritance chain.
fn abc_registry() -> PackageRegistry {
    let mut pkg = EPackage::new("abc");
    let a = EClass::new("A", EClassKind::Class);
    let mut b = EClass::new("B", EClassKind::Class);
    b.add_super_type("A").unwrap();
    let mut c = EClass::new("C", EClassKind::Class);
    c.add_super_type("B").unwrap();
    pkg.add_class(a);
    pkg.add_class(b);
    pkg.add_class(c);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

// ===========================================================================
// EClassImplTests.cpp
// ===========================================================================

#[test]
fn eclass_create_eclass_and_attribute() {
    let mut cls = EClass::new("Person", EClassKind::Class);
    let mut id = EStructuralFeature::new("id", FeatureKind::Attribute, 1, 1); // required
    id.set_type_name("EString");
    let mut age = EStructuralFeature::new("age", FeatureKind::Attribute, 0, 1);
    age.set_type_name("EInt");
    cls.add_feature(id);
    cls.add_feature(age);

    assert_eq!(cls.name(), "Person");
    assert!(!cls.is_abstract());
    assert!(!cls.is_interface());
    assert_eq!(cls.e_structural_features().len(), 2);

    let reg = PackageRegistry::new();
    assert_eq!(cls.e_all_attributes(&reg).len(), 2);
    assert!(cls.feature_by_name("id", &reg).is_some());
    let age_f = cls.feature_by_name("age", &reg).unwrap();
    assert_eq!(age_f.name(), "age");
    assert_eq!(age_f.type_name().unwrap(), "EInt");
    assert!(cls.feature_by_name("id", &reg).unwrap().is_required());
}

#[test]
fn eclass_feature_id_lookup() {
    let mut cls = EClass::new("Book", EClassKind::Class);
    cls.add_feature(tagged_attr("title", "EString", 0));
    cls.add_feature(tagged_attr("isbn", "EString", 1));

    let reg = PackageRegistry::new();
    assert_eq!(cls.feature_id_of("title", &reg), Some(0));
    assert_eq!(cls.feature_id_of("isbn", &reg), Some(1));
    assert_eq!(
        cls.feature_by_name("title", &reg).map(|f| f.feature_id()),
        Some(0)
    );
    assert_eq!(
        cls.feature_by_name("isbn", &reg).map(|f| f.feature_id()),
        Some(1)
    );
    assert!(cls.feature_id_of("none", &reg).is_none());
}

#[test]
fn eclass_is_super_type_of() {
    let mut pkg = EPackage::new("p");
    pkg.add_class(EClass::new("Parent", EClassKind::Class));
    let mut child = EClass::new("Child", EClassKind::Class);
    child.add_super_type("Parent").unwrap();
    pkg.add_class(child);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    let parent = reg.find_class("Parent").unwrap();
    let child = reg.find_class("Child").unwrap();

    // C++ parent.isSuperTypeOf(child) == true  => Rust child.is_super_type_of("Parent")
    assert!(child.is_super_type_of("Parent", &reg));
    // C++ child.isSuperTypeOf(parent) == false => Rust parent.is_super_type_of("Child")
    assert!(!parent.is_super_type_of("Child", &reg));
    // C++ 自反 parent.isSuperTypeOf(parent)==true；Rust 为严格 EMF 语义(不含自身)
    assert!(!parent.is_super_type_of("Parent", &reg));
}

#[test]
fn eclass_abstract_and_interface() {
    let mut cls = EClass::new("X", EClassKind::Class);
    cls.set_abstract(true);
    assert!(cls.is_abstract());
    cls.set_abstract(false);
    assert!(!cls.is_abstract());

    // Interface kind exposes is_interface() and implies abstract.
    let iface = EClass::new("I", EClassKind::Interface);
    assert!(iface.is_interface());
    assert!(iface.is_abstract());
    assert!(!iface.is_map_entry());
}

#[test]
fn eclass_get_structural_feature_by_id() {
    let mut cls = EClass::new("Cls", EClassKind::Class);
    cls.add_feature(tagged_attr("a1", "EString", 7));

    let reg = PackageRegistry::new();
    let got = structural_feature_by_id(&cls, 7, &reg);
    assert!(got.is_some());
    assert_eq!(got.unwrap().name(), "a1");
    assert!(structural_feature_by_id(&cls, 99, &reg).is_none());
}

#[test]
fn eclass_get_all_super_types_transitive_closure() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert!(store.e_all_super_types(&reg).is_empty()); // Store 无父类
    assert_eq!(library.e_all_super_types(&reg), ["Store"]); // 缓存同一引用未移植(每次新 Vec)
}

#[test]
fn eclass_get_all_super_types_multilevel() {
    let reg = abc_registry();
    let c = reg.find_class("C").unwrap();
    // C 的父类按 (更高父类..., 直接父类) 排列 => [A, B]
    assert_eq!(c.e_all_super_types(&reg), ["A", "B"]);
}

#[test]
fn eclass_get_all_attributes_inherits_from_parent() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert_eq!(store.e_all_attributes(&reg).len(), 3);
    let lib_attrs = library.e_all_attributes(&reg);
    assert_eq!(lib_attrs.len(), 3); // 无自 attribute，仅继承 Store 的 3 个
    let names: Vec<&str> = lib_attrs.iter().map(|f| f.name()).collect();
    assert!(names.contains(&"name"));
    assert!(names.contains(&"address"));
    assert!(names.contains(&"id"));
}

#[test]
fn eclass_get_all_references_includes_inherited() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert!(store.e_all_references(&reg).is_empty()); // Store 无 reference
    let refs = library.e_all_references(&reg);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name(), "books");
}

#[test]
fn eclass_get_all_containments_only_containment_refs() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert!(e_all_containments(&store, &reg).is_empty()); // Store 无 containment
    let conts = e_all_containments(&library, &reg);
    assert_eq!(conts.len(), 1);
    assert!(conts[0].is_containment());
    assert_eq!(conts[0].name(), "books");
}

#[test]
fn eclass_get_all_operations_inherits_from_parent() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert_eq!(get_operation_count(&store, &reg), 1); // open
    let ops = e_all_operations(&library, &reg);
    assert_eq!(ops.len(), 2); // open(继承) + close(自身)
    let names: Vec<&str> = ops.iter().map(|o| o.name()).collect();
    assert!(names.contains(&"open"));
    assert!(names.contains(&"close"));
}

#[test]
fn eclass_get_all_structural_features_includes_inherited() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert_eq!(store.e_all_structural_features(&reg).len(), 3);
    assert_eq!(feature_count(&store, &reg), 3);
    assert_eq!(library.e_all_structural_features(&reg).len(), 4); // 3 + books
    assert_eq!(feature_count(&library, &reg), 4);
}

#[test]
fn eclass_get_all_generic_super_types_transitive() {
    // C++ 仅验证不崩且 size >= 0；Rust 简化为 super-types 闭包。
    let reg = store_library_registry();
    let library = reg.find_class("Library").unwrap();
    assert_eq!(library.e_all_super_types(&reg).len(), 1);
}

#[test]
fn eclass_get_eid_attribute_finds_id_marked() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    let id_attr = e_id_attribute(&store, &reg).unwrap();
    assert_eq!(id_attr.name(), "id");
    assert_eq!(id_attr.feature_id(), 2);

    // Library 继承 Store 的 id（用 feature_id 表达 C++ 的对象同一性）
    let inherited = e_id_attribute(&library, &reg).unwrap();
    assert_eq!(inherited.name(), "id");
    assert_eq!(inherited.feature_id(), store.id_feature().unwrap());
}

#[test]
fn eclass_get_feature_count_own_and_inherited() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert_eq!(feature_count(&store, &reg), 3);
    assert_eq!(feature_count(&library, &reg), 4); // 继承 3 + 自身 1 (books)
}

#[test]
fn eclass_get_eoperation_by_operation_id() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    let open_op = operation_by_id(&library, 0, &reg).unwrap();
    assert_eq!(open_op.name(), "open");
    let close_op = operation_by_id(&library, 1, &reg).unwrap();
    assert_eq!(close_op.name(), "close");

    // store->getEOperation("open")
    let store_open = operation_by_name(&store, "open", &reg).unwrap();
    assert_eq!(store_open.name(), "open");

    // 不存在 ID 返回 None
    assert!(operation_by_id(&library, 99, &reg).is_none());

    assert_eq!(get_operation_count(&library, &reg), 2);
    assert_eq!(get_operation_count(&store, &reg), 1);
}

#[test]
fn eclass_get_override_finds_parent_method() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let mut library = reg.find_class("Library").unwrap();

    // Library 重写 open（同名，新 operationID）
    library.add_operation(tagged_op("open", 2));

    let base = operation_by_name(&store, "open", &reg).unwrap();
    let override_op = override_operation(&library, &base, &reg).unwrap();
    assert_eq!(override_op.name(), "open");
    assert_ne!(override_op.operation_id(), base.operation_id());
}

#[test]
fn eclass_get_feature_type_no_crash() {
    // 主要验证 derived getter 链没被破坏。
    let reg = store_library_registry();
    let library = reg.find_class("Library").unwrap();

    assert_eq!(library.e_all_attributes(&reg).len(), 3);
    assert_eq!(library.e_all_references(&reg).len(), 1);
    assert_eq!(get_operation_count(&library, &reg), 2);
    assert_eq!(feature_count(&library, &reg), 4);
}

#[test]
fn eclass_is_super_type_of_with_inheritance() {
    let reg = store_library_registry();
    let store = reg.find_class("Store").unwrap();
    let library = reg.find_class("Library").unwrap();

    assert!(library.is_super_type_of("Store", &reg)); // store.isSuperTypeOf(library)
    assert!(!store.is_super_type_of("Library", &reg)); // library.isSuperTypeOf(store) == false
                                                       // C++ 自反 store.isSuperTypeOf(store)==true；Rust 严格语义不含自身。
    assert!(!store.is_super_type_of("Store", &reg));
}

// ===========================================================================
// EPackageImplTests.cpp
// ===========================================================================

#[test]
fn epackage_create_and_get_classifier() {
    let mut p = EPackage::new("MyPkg");
    p.set_ns_uri("http://example.com/MyPkg");
    p.set_ns_prefix("mypkg");
    p.add_class(EClass::new("Alpha", EClassKind::Class));
    p.add_class(EClass::new("Beta", EClassKind::Class));

    assert_eq!(p.classes().len(), 2);
    assert!(p.find_class("Alpha").is_some());
    assert!(p.find_class("Beta").is_some());
    assert!(p.find_class("NotThere").is_none());
    assert_eq!(p.name(), "MyPkg");
}

#[test]
fn epackage_registered_in_registry() {
    // C++ 走全局 EPackageRegistry；Rust 用本地 PackageRegistry 的 nsURI 查询实现 put/get。
    let mut p = EPackage::new("TestPkg");
    p.set_ns_uri("http://example.com/TestPkg-zzz");
    p.set_ns_prefix("tp");
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(p));
    assert!(reg
        .package_by_ns_uri("http://example.com/TestPkg-zzz")
        .is_some());
}

#[test]
fn epackage_accessors() {
    let mut p = EPackage::new("X");
    p.set_ns_uri("u");
    p.set_ns_prefix("x");
    assert_eq!(p.name(), "X");
    assert_eq!(p.ns_uri().unwrap().to_string(), "u");
    assert_eq!(p.ns_prefix(), "x");
}

// ===========================================================================
// EcorePackageTests.cpp
// ===========================================================================

#[test]
fn ecore_package_initialize() {
    // 初始化不应抛异常；至少能取到包与内建元类。
    let pkg = emf_ecore::ecore_package::ecore_package();
    assert!(pkg.borrow().find_class("EClass").is_some());
    assert!(pkg.borrow().find_data_type("EString").is_some());
}

#[test]
fn ecore_package_meta_eclasses_non_null() {
    let pkg = emf_ecore::ecore_package::ecore_package();
    let b = pkg.borrow();
    for n in [
        "EClass",
        "EAttribute",
        "EReference",
        "EPackage",
        "EEnum",
        "EDataType",
    ] {
        assert!(b.find_class(n).is_some(), "missing meta class {n}");
    }
    assert_eq!(b.find_class("EClass").unwrap().name(), "EClass");
    assert_eq!(b.find_class("EAttribute").unwrap().name(), "EAttribute");
    assert_eq!(b.find_class("EReference").unwrap().name(), "EReference");
}

#[test]
fn ecore_package_builtin_data_types_non_null() {
    let pkg = emf_ecore::ecore_package::ecore_package();
    let b = pkg.borrow();
    assert!(b.find_data_type("EString").is_some());
    assert!(b.find_data_type("EBoolean").is_some());
    assert!(b.find_data_type("EInt").is_some());
    assert_eq!(b.find_data_type("EString").unwrap().name(), "EString");
    assert_eq!(b.find_data_type("EInt").unwrap().name(), "EInt");
    assert_eq!(b.find_data_type("EBoolean").unwrap().name(), "EBoolean");
}

#[test]
fn ecore_package_registered_to_global_registry() {
    let reg = emf_ecore::ecore_package::global();
    assert!(reg.package_by_ns_uri(emf_ecore::ECORE_NS_URI).is_some());
    assert!(reg.package("ecore").is_some());
}

#[test]
fn ecore_package_namespace_constants() {
    assert_eq!(
        emf_ecore::ECORE_NS_URI,
        "http://www.eclipse.org/emf/2002/Ecore"
    );
    assert_eq!(emf_ecore::ECORE_NS_PREFIX, "ecore");
    assert_eq!(emf_ecore::ecore_package::ECORE_NAME, "ecore");
}

#[test]
fn ecore_package_feature_id_constants() {
    assert_eq!(emf_ecore::FeatureID::ECLASS_ESUPERTYPES, 4002);
    assert_eq!(emf_ecore::FeatureID::ECLASS_ESTRUCTURALFEATURES, 4003);
    assert_eq!(emf_ecore::FeatureID::EPACKAGE_ECLASSIFIERS, 10002);
    assert_eq!(emf_ecore::FeatureID::EPACKAGE_ENSURI, 10000);
}

// ===========================================================================
// DataTypeUtilTests.cpp  (via EFactory create_from_string / convert_to_string)
// ===========================================================================

#[test]
fn datatype_estring_from_to() {
    let f = EFactory::new();
    let v = f.create_from_string("EString", "hello");
    assert_eq!(v, Val::String("hello".into()));
    assert_eq!(f.convert_to_string("EString", &v), "hello");
}

#[test]
fn datatype_eint_from_to() {
    let f = EFactory::new();
    let v = f.create_from_string("EInt", "123");
    assert_eq!(v, Val::Int(123));
    assert_eq!(f.convert_to_string("EInt", &v), "123");
}

#[test]
fn datatype_eboolean_from_to() {
    let f = EFactory::new();
    let vt = f.create_from_string("EBoolean", "true");
    let vf = f.create_from_string("EBoolean", "false");
    assert_eq!(vt, Val::Bool(true));
    assert_eq!(vf, Val::Bool(false));
    assert_eq!(f.convert_to_string("EBoolean", &vt), "true");
    assert_eq!(f.convert_to_string("EBoolean", &vf), "false");
}

#[test]
fn datatype_edouble_from_to() {
    let f = EFactory::new();
    let v = f.create_from_string("EDouble", "3.5");
    assert_eq!(v, Val::Double(3.5));
    assert_eq!(f.convert_to_string("EDouble", &v), "3.5");
}

#[test]
fn datatype_default_values() {
    use emf_ecore::datatype::default_value;
    assert_eq!(default_value("EString"), Val::String(String::new()));
    assert_eq!(default_value("EInt"), Val::Int(0));
    assert_eq!(default_value("EBoolean"), Val::Bool(false));
    assert_eq!(default_value("EDouble"), Val::Double(0.0));
    assert_eq!(default_value("ELong"), Val::Int(0));
}

#[test]
fn datatype_name_of_edata_type() {
    // C++ nameOf(dt)==dt->getName()；Rust 直接读 EDataType::name。
    let pkg = emf_ecore::ecore_package::ecore_package();
    let b = pkg.borrow();
    assert_eq!(b.find_data_type("EString").unwrap().name(), "EString");
}

// ===========================================================================
// ETypedElementImplTests.cpp  (映射 to EStructuralFeature 元数据)
// ===========================================================================

#[test]
fn ettypedelement_defaults() {
    let f = EStructuralFeature::attribute("f");
    assert_eq!(f.lower_bound(), 0);
    assert_eq!(f.upper_bound(), 1);
    assert!(f.is_ordered());
    assert!(f.is_unique());
    assert!(f.type_name().is_none()); // eType 缺省 null
}

#[test]
fn ettypedelement_set_type() {
    let mut f = EStructuralFeature::attribute("f");
    f.set_type_name("MyType");
    assert_eq!(f.type_name().unwrap(), "MyType");
}

#[test]
fn ettypedelement_is_many() {
    let feat = |upper: i32| EStructuralFeature::new("f", FeatureKind::Attribute, 0, upper);
    assert!(!feat(1).is_many()); // 缺省 upper=1 => 单值
    assert!(feat(2).is_many()); // upper=2 => many
    assert!(feat(-1).is_many()); // upper=-1 (MANY) => many
                                 // 说明: Rust is_many() 语义为 upper != 1；而 C++ 为 upper==-1 || upper>1，
                                 // 因此在 upper==0 处两者不一致(C++ 视为单值, Rust 视为 many)，此处不额外断言。
}

#[test]
fn ettypedelement_is_required() {
    assert!(EStructuralFeature::new("f", FeatureKind::Attribute, 1, 1).is_required());
    assert!(!EStructuralFeature::attribute("f").is_required()); // lower 缺省 0
}

#[test]
fn ettypedelement_reflection_eget_eset() {
    // 对齐 ETypedElement_EGet / ESet / EIsSet / EUnset：通过 DynamicEObject 反射
    // 读写 lower/upper/ordered/unique/eType 这类 bound/type 属性。
    let mut pkg = EPackage::new("et");
    let mut et = EClass::new("ET", EClassKind::Class);
    et.add_feature(tagged_attr("eLowerBound", "EInt", 0));
    et.add_feature(tagged_attr("eUpperBound", "EInt", 1));
    et.add_feature(tagged_attr("eOrdered", "EBoolean", 2));
    et.add_feature(tagged_attr("eUnique", "EBoolean", 3));
    et.add_feature(tagged_attr("eType", "EString", 4));
    pkg.add_class(et);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    let et = reg.find_class("ET").unwrap();

    let mut obj = DynamicEObject::new_in(et, reg);
    // 缺省：未 set
    assert!(!obj.e_is_set_by_name("eLowerBound").unwrap());

    assert!(obj.e_set_by_name("eLowerBound", Val::Int(2)));
    assert!(obj.e_set_by_name("eUpperBound", Val::Int(-1)));
    assert!(obj.e_set_by_name("eOrdered", Val::Bool(false)));
    assert!(obj.e_set_by_name("eUnique", Val::Bool(false)));
    assert!(obj.e_set_by_name("eType", Val::String("EString".into())));

    assert_eq!(obj.e_get_by_name("eLowerBound").unwrap().as_int(), Some(2));
    assert_eq!(obj.e_get_by_name("eUpperBound").unwrap().as_int(), Some(-1));
    assert_eq!(
        obj.e_get_by_name("eOrdered").unwrap().as_bool(),
        Some(false)
    );
    assert_eq!(obj.e_get_by_name("eUnique").unwrap().as_bool(), Some(false));
    assert_eq!(
        obj.e_get_by_name("eType")
            .and_then(|v| v.as_str().map(String::from)),
        Some("EString".into())
    );

    assert!(obj.e_is_set_by_name("eUpperBound").unwrap());

    // eUnset 恢复缺省
    assert!(obj.e_unset_by_name("eUpperBound"));
    assert!(!obj.e_is_set_by_name("eUpperBound").unwrap());
    assert!(obj.e_get_by_name("eUpperBound").is_some());
}
