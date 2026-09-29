//! C++ ↔ Rust parity suite for `emf-artop-runtime`.
//!
//! Mirrors, one-to-one, the 18 C++ cases in
//! `artop-cpp/cpp/emf-cpp/emf-artop/emf-artop-runtime/tests/test_main.cpp`
//! (the reference/oracle). The Rust test names are prefixed with the C++
//! function names so `tools/conformance/cases_artop_runtime.tsv` can map them
//! and `compare.py` can verify behaviour equivalence.

use std::rc::Rc;

use emf_common::resource::ResourceHandle;
use emf_common::uri::Uri;
use emf_common::value::Val;
use emf_ecore::PackageRegistry;
use emf_xmi::{XMIResource, XMIResourceSet};

use artop_runtime::version_data::AutosarMetaModelVersionData;
use artop_runtime::{
    AutosarReleaseDescriptor, AutosarResource, AutosarResourceFactory, AutosarXMLResource,
    IdentifiableUtil,
};

fn release_448(id: &str) -> AutosarReleaseDescriptor {
    AutosarReleaseDescriptor::new(id.to_string(), AutosarMetaModelVersionData::new(4, 4, 8))
}

// 用例 1: AutosarMetaModelVersionData 基础构造
#[test]
fn test_version_data_basic() {
    let v = AutosarMetaModelVersionData::new(4, 4, 8);
    assert_eq!(v.major(), 4);
    assert_eq!(v.minor(), 4);
    assert_eq!(v.revision(), 8);
    assert!(v.is_new_version());
    assert_eq!(v.to_string(), "4.4.8");
}

// 用例 2: 从字符串解析
#[test]
fn test_version_data_parse() {
    let v =
        AutosarMetaModelVersionData::create_from_canonical_version_number_string("4.4.8").unwrap();
    assert_eq!((v.major(), v.minor(), v.revision()), (4, 4, 8));

    let v2 =
        AutosarMetaModelVersionData::create_from_canonical_version_number_string("4.2.1").unwrap();
    assert_eq!(v2.major(), 4);
    assert!(!v2.is_new_version());

    let v3 =
        AutosarMetaModelVersionData::create_from_canonical_version_number_string("4.0").unwrap();
    assert_eq!((v3.major(), v3.minor(), v3.revision()), (4, 0, 0));
}

// 用例 3: canonical number 编/解码
#[test]
fn test_version_canonical() {
    let v = AutosarMetaModelVersionData::new(4, 4, 8);
    assert_ne!(v.canonical_version_number(), 0);
}

// 用例 4: schema version number string
#[test]
fn test_schema_version_string() {
    let v448 = AutosarMetaModelVersionData::new(4, 4, 8);
    assert_eq!(v448.schema_version_number_string("-"), "4-4-8");

    let v421 = AutosarMetaModelVersionData::new(4, 2, 1);
    assert_eq!(v421.schema_version_number_string(""), "00042");
}

// 用例 5: AutosarReleaseDescriptor 基础属性
#[test]
fn test_release_descriptor_basic() {
    let desc = release_448("org.artop.aal.autosar448");
    assert_eq!(desc.id(), "org.artop.aal.autosar448");
    assert_eq!(desc.autosar_version_data().major(), 4);
    assert_eq!(desc.base_namespace(), "http://autosar.org/schema/r4.0");
}

// 用例 6: schema location 拼接
#[test]
fn test_release_schema_location() {
    let desc = release_448("org.artop.aal.autosar448");
    let sl = desc.schema_location();
    assert!(sl.contains("http://autosar.org/schema/r4.0"));
    assert!(sl.contains("AUTOSAR_4-4-8.xsd"));

    let desc40 = AutosarReleaseDescriptor::new(
        "org.artop.aal.autosar40".to_string(),
        AutosarMetaModelVersionData::new(4, 2, 1),
    );
    assert!(desc40.schema_location().contains("AUTOSAR_00042.xsd"));
}

// 用例 7: matchesSchemaLocation
#[test]
fn test_release_matches() {
    let desc = release_448("org.artop.aal.autosar448");
    assert!(desc.matches_schema_location("http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd"));
    assert!(desc.matches_schema_location("http://autosar.org/schema/r4.0"));
    assert!(!desc.matches_schema_location("http://example.com/other"));
}

// 用例 8: descriptor 兼容性比较
#[test]
fn test_release_compare() {
    let d448 = release_448("a");
    let d447 =
        AutosarReleaseDescriptor::new("b".to_string(), AutosarMetaModelVersionData::new(4, 4, 7));
    assert!(d447.compare_to(&d448) < 0);
    assert!(d448.compare_to(&d447) > 0);
}

// 用例 9: AutosarResource 基础
#[test]
fn test_resource_basic() {
    let desc = release_448("x");
    let mut res = AutosarResource::with_release(
        Uri::parse("file:///tmp/test.arxml"),
        PackageRegistry::default(),
        Some(desc),
    );
    assert!(res.autosar_release().is_some());
    assert_eq!(res.uri().to_string(), "file:///tmp/test.arxml");
    assert!(res.schema_location().is_empty());
    res.set_schema_location("http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd");
    assert!(!res.schema_location().is_empty());
}

// 用例 10: AutosarXMLResource createXMLLoad/createXMLSave
#[test]
fn test_xml_resource_factory_methods() {
    let res = AutosarXMLResource::new(
        Uri::parse("file:///tmp/test.arxml"),
        PackageRegistry::default(),
    );
    // Rust has no nullable trait-object handles: the C++ `!= nullptr` checks
    // become "the factory methods return usable handles".
    let _load = res.create_xml_load();
    let _save = res.create_xml_save();
    let _helper = res.create_xml_helper();
}

// 用例 11: AutosarResourceFactory 注入 creator
#[test]
fn test_resource_factory_creator() {
    let desc = release_448("x");
    let mut factory = AutosarResourceFactory::new(Some(desc.clone()));
    factory.set_resource_creator(Rc::new(move |uri: &Uri, registry: &PackageRegistry| {
        Box::new(AutosarResource::with_release(
            uri.clone(),
            registry.clone(),
            Some(desc.clone()),
        )) as Box<dyn ResourceHandle>
    }));
    let res = factory.create_resource(Uri::parse("file:///tmp/t.arxml"));
    assert_eq!(res.uri().to_string(), "file:///tmp/t.arxml");
    assert!(res.as_any().downcast_ref::<AutosarResource>().is_some());
}

// 用例 12: initResource 注入 schema location
#[test]
fn test_init_resource_sets_xsi() {
    let factory = AutosarResourceFactory::new(Some(release_448("x")));
    let mut res = XMIResource::new(
        Uri::parse("file:///tmp/t.arxml"),
        PackageRegistry::default(),
    );
    factory.init_resource(&mut res);
    assert!(res.get_xsi_schema_location().contains("AUTOSAR_4-4-8.xsd"));
}

// 用例 13: schema location catalog
#[test]
fn test_schema_catalog() {
    let factory = AutosarResourceFactory::new(Some(release_448("x")));
    let cat = factory.create_schema_location_catalog();
    assert!(!cat.is_empty());
    assert!(cat.contains_key("http://autosar.org/schema/r4.0"));
}

// 用例 14: IdentifiableUtil 在无对象时安全
#[test]
fn test_identifiable_util_null() {
    assert!(IdentifiableUtil::get_short_name(None).is_empty());
    assert!(!IdentifiableUtil::has_short_name(None));
    IdentifiableUtil::set_short_name(None, "x"); // must not panic
}

// 用例 15: descriptor INSTANCE 兜底
#[test]
fn test_descriptor_instance() {
    assert_eq!(
        AutosarReleaseDescriptor::instance().id(),
        "org.artop.aal.autosar.release"
    );
}

// 用例 16: 元模型注册 —— 加载内置 autosar40.ecore
#[test]
fn test_metamodel_registration() {
    let pkg = AutosarResourceFactory::register_default_autosar40_metamodel();
    assert_eq!(pkg.borrow().name(), "autosar40");
    assert_eq!(
        pkg.borrow().ns_uri().unwrap().to_string(),
        "http://autosar.org/schema/r4.0"
    );
    // 幂等：再调用一次返回同一对象
    let pkg2 = AutosarResourceFactory::register_default_autosar40_metamodel();
    assert!(Rc::ptr_eq(&pkg, &pkg2));
    // EPackageRegistry 已注册
    assert!(emf_ecore::ecore_package::global_get("http://autosar.org/schema/r4.0").is_some());
    // 验证 AUTOSAR EClass 已加载
    assert!(pkg.borrow().find_class("AUTOSAR").is_some());
}

// 用例 17: arxml 实例化为 EObject —— <AUTOSAR> 根元素
#[test]
fn test_arxml_instantiation_root() {
    AutosarResourceFactory::register_default_autosar40_metamodel();
    let arxml = "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
                 <SHORT-NAME>rootAutosar</SHORT-NAME>\
                 </AUTOSAR>";
    let mut set = XMIResourceSet::new(emf_ecore::ecore_package::global());
    let res = set.create_resource(Uri::parse("file:///tmp/test.arxml"));
    res.borrow_mut().load_from_string(arxml).unwrap();
    let contents = res.borrow().resource().contents().to_vec();
    assert!(!contents.is_empty());
    let root = &contents[0];
    assert_eq!(root.borrow().e_class(), "AUTOSAR");
}

// 用例 18: arxml containment 递归 —— AR-PACKAGE 子结构
#[test]
fn test_arxml_containment_arpackage() {
    AutosarResourceFactory::register_default_autosar40_metamodel();
    let arxml = "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
                 <AR-PACKAGE><SHORT-NAME>pkg1</SHORT-NAME></AR-PACKAGE>\
                 <AR-PACKAGE>\
                 <SHORT-NAME>pkg2</SHORT-NAME>\
                 <AR-PACKAGES><AR-PACKAGE><SHORT-NAME>sub1</SHORT-NAME></AR-PACKAGE></AR-PACKAGES>\
                 </AR-PACKAGE>\
                 </AUTOSAR>";
    let mut set = XMIResourceSet::new(emf_ecore::ecore_package::global());
    let res = set.create_resource(Uri::parse("file:///tmp/test2.arxml"));
    res.borrow_mut().load_from_string(arxml).unwrap();
    let contents = res.borrow().resource().contents().to_vec();
    assert!(!contents.is_empty());
    let root = contents[0].clone();

    let val = root.borrow().e_get("AR-PACKAGE").expect("AR-PACKAGE list");
    let items = match val {
        Val::List(items) => items,
        other => panic!("expected AR-PACKAGE list, got {other:?}"),
    };
    assert_eq!(items.len(), 2);
    let pkg1 = items[0].as_object().expect("pkg1 object").clone();
    assert_eq!(
        pkg1.borrow().e_get("SHORT-NAME"),
        Some(Val::String("pkg1".into()))
    );
}
