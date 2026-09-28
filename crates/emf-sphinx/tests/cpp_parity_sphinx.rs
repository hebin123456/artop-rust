//! C++ parity suite: emf-sphinx.
//!
//! Ports `EcoreResourceUtilTests.cpp`, `MetaModelDescriptorTests.cpp`,
//! `OrderedFeatureMapTests.cpp`, `ResourceTests.cpp` and `ScopingTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-sphinx/tests/` against the Rust `emf-sphinx`
//! crate. (`ExtendedResourceTests.cpp`, `ModelDescriptorTests.cpp` and
//! `ProxyHelperTests.cpp` are empty in C++ and have no assertions to port.)
//!
//! Behaviour notes (see `docs/PARITY_TRACKER.md`):
//!   - Rust has no `nullptr`: C++ `nullptr` arguments map to `Option::None`.
//!   - C++ object-identity comparisons on `IMetaModelDescriptor*` /
//!     `IModelConverter*` / providers map to `Rc::ptr_eq`.
//!   - The registry state lives in thread-locals, so tests are isolated per
//!     thread (libtest runs each test on its own thread).

use std::rc::Rc;

use emf_common::resource::{Resource, ResourceHandle};
use emf_common::uri::Uri;
use emf_ecore::{EStructuralFeature, PackageRegistry};
use emf_sphinx::ecore::{FeatureMapEntry, OrderedFeatureMap};
use emf_sphinx::metamodel::{
    AbstractMetaModelDescriptor, MetaModelDescriptor, MetaModelDescriptorRegistry,
    MetaModelVersionData,
};
use emf_sphinx::resource::{
    ExtendedBasicExtendedMetaData, ModelConverter, ModelConverterRegistry, SchemaLocationUriHandler,
};
use emf_sphinx::scoping::{
    FileResourceScope, FileResourceScopeProvider, ResourceScope, ResourceScopeProvider,
    ResourceScopeProviderRegistry,
};
use emf_sphinx::util::{EcoreResourceUtil, ResourceOptions};
use emf_xmi::XMIResource;

// ===========================================================================
// EcoreResourceUtilTests.cpp (20)
// ===========================================================================

#[test]
fn ecore_resource_util_convert_to_absolute_file_uri() {
    let rel = Uri::parse("relative/path.xmi");
    let abs = EcoreResourceUtil::convert_to_absolute_file_uri(&rel);
    assert!(!abs.to_string().is_empty());
}

#[test]
fn ecore_resource_util_convert_to_platform_resource_uri() {
    let uri = Uri::parse("file:///tmp/test.xmi");
    let conv = EcoreResourceUtil::convert_to_platform_resource_uri(&uri);
    assert!(!conv.to_string().is_empty());
}

#[test]
fn ecore_resource_util_exists() {
    assert!(!EcoreResourceUtil::exists(&Uri::parse(
        "file:///nonexistent/path.xmi"
    )));
}

#[test]
fn ecore_resource_util_get_uri_null_object() {
    let u = EcoreResourceUtil::get_uri(None);
    assert_eq!(u.to_string(), "");
}

#[test]
fn ecore_resource_util_normalize_uri_fragment_null() {
    assert_eq!(
        EcoreResourceUtil::normalize_uri_fragment(None, "foo/bar"),
        "foo/bar"
    );
}

#[test]
fn ecore_resource_util_read_model_namespace_null() {
    assert_eq!(EcoreResourceUtil::read_model_namespace(None), "");
}

#[test]
fn ecore_resource_util_read_target_namespace_null() {
    assert_eq!(EcoreResourceUtil::read_target_namespace(None), "");
}

#[test]
fn ecore_resource_util_read_root_element_comments() {
    assert!(EcoreResourceUtil::read_root_element_comments(None).is_empty());
}

#[test]
fn ecore_resource_util_read_schema_location_entries() {
    assert!(EcoreResourceUtil::read_schema_location_entries(None).is_empty());
}

#[test]
fn ecore_resource_util_default_load_options() {
    let m = EcoreResourceUtil::get_default_load_options();
    assert!(!m.is_empty());
}

#[test]
fn ecore_resource_util_default_save_options() {
    assert!(EcoreResourceUtil::get_default_save_options().is_empty());
}

#[test]
fn ecore_resource_util_get_model_root_null() {
    assert!(EcoreResourceUtil::get_model_root(None).is_none());
}

#[test]
fn ecore_resource_util_is_resource_loaded_null() {
    assert!(!EcoreResourceUtil::is_resource_loaded(
        None,
        &Uri::parse("file:///tmp/test.xmi")
    ));
}

#[test]
fn ecore_resource_util_get_model_name_null() {
    assert_eq!(EcoreResourceUtil::get_model_name(None), "");
}

#[test]
fn ecore_resource_util_load_resource_null() {
    let opts = ResourceOptions::new();
    let uri = Uri::parse("file:///tmp/test.xmi");
    let r = EcoreResourceUtil::load_resource(None, &uri, &opts).expect("no error");
    assert!(r.is_none());
}

#[test]
fn ecore_resource_util_load_eobject_null() {
    let uri = Uri::parse("file:///tmp/test.xmi");
    assert!(EcoreResourceUtil::load_eobject(None, &uri).is_none());
}

#[test]
fn ecore_resource_util_get_eobject_null() {
    let uri = Uri::parse("file:///tmp/test.xmi");
    assert!(EcoreResourceUtil::get_eobject(None, &uri).is_none());
}

#[test]
fn ecore_resource_util_add_new_model_resource_null() {
    let uri = Uri::parse("file:///tmp/test.xmi");
    let r = EcoreResourceUtil::add_new_model_resource(None, &uri, "xmi", None);
    assert!(r.is_none());
}

#[test]
fn ecore_resource_util_save_model_resource_null() {
    let opts = ResourceOptions::new();
    assert!(EcoreResourceUtil::save_model_resource(None, &opts).is_ok());
}

#[test]
fn ecore_resource_util_unload_resource_null() {
    EcoreResourceUtil::unload_resource(None, false);
    EcoreResourceUtil::unload_resource_from_set(None, &Uri::parse("file:///tmp/test.xmi"), false);
}

// ===========================================================================
// MetaModelDescriptorTests.cpp (18)
// ===========================================================================

#[test]
fn meta_model_descriptor_basic3arg() {
    let d = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:my.mm", "MyMetaModel");
    assert_eq!(d.get_identifier(), "urn:my.mm");
    assert_eq!(d.get_name(), "MyMetaModel");
    assert_eq!(d.namespace(), "urn:my.mm");
}

#[test]
fn meta_model_descriptor_multi_epackage() {
    let d = AbstractMetaModelDescriptor::with_pattern(
        "urn:my.mm",
        "urn:my.mm",
        "v[0-9]+",
        "MyMetaModel",
    );
    assert_eq!(d.e_package_ns_uri_pattern(), "urn:my.mm/v[0-9]+");
}

#[test]
fn meta_model_descriptor_with_version() {
    let vd = MetaModelVersionData::new("v1", "v1", "Version1");
    let d = AbstractMetaModelDescriptor::with_version("urn:my.mm", "urn:my.mm", vd);
    assert_eq!(d.namespace(), "urn:my.mm/v1");
    assert_eq!(d.get_name(), "Version1");
}

#[test]
fn meta_model_descriptor_matches_namespace() {
    let d = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:my.mm", "MyMetaModel");
    assert!(d.matches_namespace("urn:my.mm"));
    assert!(!d.matches_namespace("urn:other.mm"));
}

#[test]
fn meta_model_descriptor_matches_epackage_pattern() {
    let d = AbstractMetaModelDescriptor::with_pattern(
        "urn:my.mm",
        "urn:my.mm",
        "v[0-9]+",
        "MyMetaModel",
    );
    assert!(d.matches_epackage_ns_uri_pattern("urn:my.mm/v1"));
    assert!(d.matches_epackage_ns_uri_pattern("urn:my.mm/v42"));
    assert!(!d.matches_epackage_ns_uri_pattern("urn:my.mm/stable"));
    assert!(!d.matches_epackage_ns_uri_pattern("urn:other.mm/v1"));
}

#[test]
fn meta_model_descriptor_equals() {
    let a = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:my.mm", "A");
    let b = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:other.mm", "B");
    assert!(a.equals(Some(&b)));
    let c = AbstractMetaModelDescriptor::new("urn:other", "urn:other", "C");
    assert!(!a.equals(Some(&c)));
    assert!(!a.equals(None));
}

#[test]
fn meta_model_descriptor_hash_code() {
    let a = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:my.mm", "A");
    let b = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:other", "B");
    assert_eq!(a.hash_code(), b.hash_code());
}

#[test]
fn meta_model_descriptor_ordinal() {
    let vd = MetaModelVersionData::with_ordinal("v1", "v1", "v1", 5);
    let d = AbstractMetaModelDescriptor::with_version("urn:my.mm", "urn:my.mm", vd);
    assert_eq!(d.ordinal(), 5);
}

#[test]
fn meta_model_descriptor_compatible() {
    let mut d = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:my.mm", "X");
    let c1: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::with_pattern(
        "urn:my.mm/v1",
        "urn:my.mm",
        "v1",
        "v1",
    ));
    let c2: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::with_pattern(
        "urn:my.mm/v2",
        "urn:my.mm",
        "v2",
        "v2",
    ));
    d.add_compatible_resource_version_descriptor(c1);
    d.add_compatible_resource_version_descriptor(c2);
    assert_eq!(d.compatible_resource_version_descriptors().len(), 2);
}

#[test]
fn meta_model_descriptor_set_version_data() {
    let mut d = AbstractMetaModelDescriptor::new("urn:my.mm", "urn:my.mm", "X");
    assert_eq!(d.namespace(), "urn:my.mm");
    let vd = MetaModelVersionData::new("v2", "v2", "v2");
    d.set_version_data(vd);
    assert_eq!(d.namespace(), "urn:my.mm/v2");
    assert_eq!(d.get_name(), "v2");
}

#[test]
fn meta_model_version_data_fields() {
    let vd = MetaModelVersionData::new("post", "v[0-9]+", "Ver1");
    assert_eq!(vd.ns_postfix(), "post");
    assert_eq!(vd.e_package_ns_uri_postfix_pattern(), "v[0-9]+");
    assert_eq!(vd.name(), "Ver1");
    assert_eq!(vd.ordinal(), -1);
}

#[test]
fn meta_model_version_data_equals() {
    let a = MetaModelVersionData::new("v1", "v1", "v1");
    let b = MetaModelVersionData::new("v1", "v1", "v1");
    let c = MetaModelVersionData::new("v2", "v2", "v2");
    assert!(a.equals(&b));
    assert!(!a.equals(&c));
}

#[test]
fn meta_model_descriptor_registry_register_and_lookup() {
    let reg = MetaModelDescriptorRegistry::instance();
    reg.clear();
    let d: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::new(
        "urn:my.mm",
        "urn:my.mm",
        "MyMetaModel",
    ));
    reg.register_descriptor(Rc::clone(&d));
    let by_name = reg.get_descriptor("urn:my.mm").expect("registered");
    assert!(Rc::ptr_eq(&by_name, &d));
    let by_uri = reg
        .get_descriptor_uri(&Uri::parse("urn:my.mm"))
        .expect("registered by uri");
    assert!(Rc::ptr_eq(&by_uri, &d));
    reg.unregister_descriptor(&d);
    assert!(reg.get_descriptor("urn:my.mm").is_none());
}

#[test]
fn meta_model_descriptor_registry_lookup_by_object() {
    let reg = MetaModelDescriptorRegistry::instance();
    reg.clear();
    let d: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::new(
        "urn:my.mm",
        "urn:my.mm",
        "MyMetaModel",
    ));
    reg.register_descriptor(Rc::clone(&d));
    assert!(reg.get_descriptor_for_object(None).is_none());
    assert!(reg.get_descriptor_for_resource(None).is_none());
    reg.unregister_descriptor(&d);
}

#[test]
fn meta_model_descriptor_registry_no_duplicate() {
    let reg = MetaModelDescriptorRegistry::instance();
    reg.clear();
    let d: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::new(
        "urn:my.mm",
        "urn:my.mm",
        "X",
    ));
    reg.register_descriptor(Rc::clone(&d));
    reg.register_descriptor(Rc::clone(&d));
    assert_eq!(reg.all().len(), 1);
    reg.unregister_descriptor(&d);
}

#[test]
fn meta_model_descriptor_registry_multiple() {
    let reg = MetaModelDescriptorRegistry::instance();
    reg.clear();
    let d1: Rc<dyn MetaModelDescriptor> =
        Rc::new(AbstractMetaModelDescriptor::new("urn:a", "urn:a", "A"));
    let d2: Rc<dyn MetaModelDescriptor> =
        Rc::new(AbstractMetaModelDescriptor::new("urn:b", "urn:b", "B"));
    reg.register_descriptor(Rc::clone(&d1));
    reg.register_descriptor(Rc::clone(&d2));
    assert_eq!(reg.all().len(), 2);
    assert_eq!(reg.keys().len(), 2);
    reg.clear();
}

#[test]
fn meta_model_descriptor_registry_target_and_old() {
    let reg = MetaModelDescriptorRegistry::instance();
    reg.clear();
    let t: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::new(
        "urn:target",
        "urn:target",
        "T",
    ));
    let o: Rc<dyn MetaModelDescriptor> =
        Rc::new(AbstractMetaModelDescriptor::new("urn:old", "urn:old", "O"));
    reg.register_descriptor(Rc::clone(&t));
    reg.register_descriptor(Rc::clone(&o));
    let target = reg
        .get_target_descriptor(&Uri::parse("urn:target"))
        .expect("target");
    assert!(Rc::ptr_eq(&target, &t));
    let old = reg.get_old_descriptor(&Uri::parse("urn:old")).expect("old");
    assert!(Rc::ptr_eq(&old, &o));
    reg.clear();
}

#[test]
fn meta_model_descriptor_registry_lookup_by_resource() {
    let reg = MetaModelDescriptorRegistry::instance();
    reg.clear();
    let d: Rc<dyn MetaModelDescriptor> = Rc::new(AbstractMetaModelDescriptor::new(
        "urn:my.mm",
        "urn:my.mm",
        "X",
    ));
    reg.register_descriptor(Rc::clone(&d));
    let r = Resource::new(Uri::parse("file:///tmp/x.xmi"));
    assert!(reg.get_descriptor_for_resource(Some(&r)).is_none());
    reg.clear();
}

// ===========================================================================
// OrderedFeatureMapTests.cpp (7)
// ===========================================================================

/// A structural feature carrying just a feature id (for ordering tests).
fn feature_with_id(id: i32) -> Rc<EStructuralFeature> {
    let mut f = EStructuralFeature::attribute("f");
    f.set_feature_id(id);
    Rc::new(f)
}

#[test]
fn ordered_feature_map_initially_empty() {
    let m = OrderedFeatureMap::new();
    assert!(m.is_empty());
    assert_eq!(m.size(), 0);
}

#[test]
fn ordered_feature_map_add_ordered_by_feature_id() {
    let mut m = OrderedFeatureMap::new();
    let f3 = feature_with_id(3);
    let f1 = feature_with_id(1);
    let f2 = feature_with_id(2);
    m.add(Some(Rc::clone(&f3)), None, 0);
    m.add(Some(Rc::clone(&f1)), None, 0);
    m.add(Some(Rc::clone(&f2)), None, 0);
    assert_eq!(m.size(), 3);
    let e = m.entries();
    assert_eq!(e[0].feature.as_ref().unwrap().feature_id(), 1);
    assert_eq!(e[1].feature.as_ref().unwrap().feature_id(), 2);
    assert_eq!(e[2].feature.as_ref().unwrap().feature_id(), 3);
}

#[test]
fn ordered_feature_map_same_feature_id_ordered_by_index() {
    let mut m = OrderedFeatureMap::new();
    let f = feature_with_id(5);
    m.add(Some(Rc::clone(&f)), None, 2);
    m.add(Some(Rc::clone(&f)), None, 0);
    m.add(Some(Rc::clone(&f)), None, 1);
    let e = m.entries();
    assert_eq!(m.size(), 3);
    assert_eq!(e[0].index, 0);
    assert_eq!(e[1].index, 1);
    assert_eq!(e[2].index, 2);
}

#[test]
fn ordered_feature_map_get_by_feature() {
    let mut m = OrderedFeatureMap::new();
    let f1 = feature_with_id(1);
    let f2 = feature_with_id(2);
    m.add(Some(Rc::clone(&f1)), None, 0);
    m.add(Some(Rc::clone(&f2)), None, 0);
    m.add(Some(Rc::clone(&f1)), None, 1);
    m.add(Some(Rc::clone(&f2)), None, 1);
    let r1 = m.get(&f1);
    let r2 = m.get(&f2);
    assert_eq!(r1.len(), 2);
    assert_eq!(r2.len(), 2);
    assert!(r1[0].feature.as_ref().is_some_and(|f| Rc::ptr_eq(f, &f1)));
    assert!(r2[0].feature.as_ref().is_some_and(|f| Rc::ptr_eq(f, &f2)));
}

#[test]
fn ordered_feature_map_clear() {
    let mut m = OrderedFeatureMap::new();
    let f = feature_with_id(1);
    m.add(Some(Rc::clone(&f)), None, 0);
    m.add(Some(Rc::clone(&f)), None, 1);
    assert_eq!(m.size(), 2);
    m.clear();
    assert!(m.is_empty());
    assert_eq!(m.size(), 0);
}

#[test]
fn ordered_feature_map_default_order() {
    let empty = FeatureMapEntry::default();
    assert_eq!(OrderedFeatureMap::default_order(&empty), -1);
    let f = feature_with_id(42);
    let entry = FeatureMapEntry::new(Some(f), None, 0);
    assert_eq!(OrderedFeatureMap::default_order(&entry), 42);
}

#[test]
fn ordered_feature_map_size_grows() {
    let mut m = OrderedFeatureMap::new();
    let f1 = feature_with_id(10);
    let f2 = feature_with_id(20);
    assert_eq!(m.size(), 0);
    m.add(Some(Rc::clone(&f1)), None, 0);
    assert_eq!(m.size(), 1);
    m.add(Some(Rc::clone(&f2)), None, 0);
    assert_eq!(m.size(), 2);
}

// ===========================================================================
// ResourceTests.cpp (13) — schema location / extended meta-data / converters
// ===========================================================================

#[test]
fn schema_location_uri_handler_parse_simple() {
    let h = SchemaLocationUriHandler::new();
    let m = h.parse_schema_location("ns1 uri1 ns2 uri2");
    assert_eq!(m.len(), 2);
    assert_eq!(m["ns1"], "uri1");
    assert_eq!(m["ns2"], "uri2");
}

#[test]
fn schema_location_uri_handler_parse_empty() {
    let h = SchemaLocationUriHandler::new();
    assert!(h.parse_schema_location("").is_empty());
}

#[test]
fn schema_location_uri_handler_parse_single() {
    let h = SchemaLocationUriHandler::new();
    let m = h.parse_schema_location("ns1 uri1");
    assert_eq!(m.len(), 1);
    assert_eq!(m["ns1"], "uri1");
}

#[test]
fn schema_location_uri_handler_parse_odd_tokens() {
    let h = SchemaLocationUriHandler::new();
    assert!(h.parse_schema_location("ns1 uri1 ns2").is_empty());
}

#[test]
fn schema_location_uri_handler_parse_multiple_spaces() {
    let h = SchemaLocationUriHandler::new();
    let m = h.parse_schema_location("  ns1   uri1   ns2   uri2  ");
    assert_eq!(m.len(), 2);
    assert_eq!(m["ns1"], "uri1");
    assert_eq!(m["ns2"], "uri2");
}

#[test]
fn schema_location_uri_handler_get_null() {
    let h = SchemaLocationUriHandler::new();
    assert_eq!(h.get_schema_location(None), "");
}

#[test]
fn schema_location_uri_handler_get_from_resource() {
    let h = SchemaLocationUriHandler::new();
    let mut r = XMIResource::new(Uri::parse("file:///tmp/r.xmi"), PackageRegistry::new());
    r.set_xsi_schema_location("ns1 uri1");
    assert_eq!(h.get_schema_location(Some(&r)), "ns1 uri1");
}

#[test]
fn extended_basic_extended_meta_data_cache_key() {
    let m = ExtendedBasicExtendedMetaData::instance();
    assert_eq!(m.get_cache_key("ns", "loc"), "ns|loc");
}

#[test]
fn extended_basic_extended_meta_data_cache_key_empty() {
    let m = ExtendedBasicExtendedMetaData::instance();
    assert_eq!(m.get_cache_key("", "loc"), "loc");
}

/// A minimal descriptor used by the model-converter tests.
struct SrcDesc;

impl MetaModelDescriptor for SrcDesc {
    fn identifier(&self) -> String {
        "src".into()
    }
    fn namespace_uri(&self) -> Uri {
        Uri::parse("src")
    }
    fn name(&self) -> String {
        "Src".into()
    }
    fn base_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>> {
        None
    }
    fn custom_uri_scheme(&self) -> String {
        String::new()
    }
    fn ordinal(&self) -> i32 {
        0
    }
    fn e_package_ns_uri_pattern(&self) -> String {
        "src".into()
    }
    fn matches_namespace(&self, ns: &str) -> bool {
        ns == "src"
    }
    fn matches_epackage_ns_uri_pattern(&self, ns: &str) -> bool {
        ns == "src"
    }
    fn equals(&self, other: Option<&dyn MetaModelDescriptor>) -> bool {
        other.is_some_and(|o| o.identifier() == "src")
    }
    fn compatible_resource_version_descriptors(&self) -> Vec<Rc<dyn MetaModelDescriptor>> {
        Vec::new()
    }
}

/// The target-side descriptor used by the model-converter tests.
struct TgtDesc;

impl MetaModelDescriptor for TgtDesc {
    fn identifier(&self) -> String {
        "tgt".into()
    }
    fn namespace_uri(&self) -> Uri {
        Uri::parse("tgt")
    }
    fn name(&self) -> String {
        "Tgt".into()
    }
    fn base_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>> {
        None
    }
    fn custom_uri_scheme(&self) -> String {
        String::new()
    }
    fn ordinal(&self) -> i32 {
        0
    }
    fn e_package_ns_uri_pattern(&self) -> String {
        "tgt".into()
    }
    fn matches_namespace(&self, ns: &str) -> bool {
        ns == "tgt"
    }
    fn matches_epackage_ns_uri_pattern(&self, ns: &str) -> bool {
        ns == "tgt"
    }
    fn equals(&self, other: Option<&dyn MetaModelDescriptor>) -> bool {
        other.is_some_and(|o| o.identifier() == "tgt")
    }
    fn compatible_resource_version_descriptors(&self) -> Vec<Rc<dyn MetaModelDescriptor>> {
        Vec::new()
    }
}

/// A converter that carries a source/target descriptor and converts nothing.
struct MockModelConverter {
    id: String,
    src: Rc<dyn MetaModelDescriptor>,
    tgt: Rc<dyn MetaModelDescriptor>,
}

impl MockModelConverter {
    fn new(id: &str, src: Rc<dyn MetaModelDescriptor>, tgt: Rc<dyn MetaModelDescriptor>) -> Self {
        Self {
            id: id.to_string(),
            src,
            tgt,
        }
    }
}

impl ModelConverter for MockModelConverter {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn source_meta_model_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>> {
        Some(Rc::clone(&self.src))
    }
    fn target_meta_model_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>> {
        Some(Rc::clone(&self.tgt))
    }
    fn convert(
        &self,
        _source: &Resource,
        _target_content_type: &str,
    ) -> Option<Box<dyn ResourceHandle>> {
        None
    }
}

fn mock_converter() -> Rc<dyn ModelConverter> {
    let src: Rc<dyn MetaModelDescriptor> = Rc::new(SrcDesc);
    let tgt: Rc<dyn MetaModelDescriptor> = Rc::new(TgtDesc);
    Rc::new(MockModelConverter::new("c1", src, tgt))
}

#[test]
fn model_converter_registry_add_and_find() {
    let reg = ModelConverterRegistry::instance();
    let c = mock_converter();
    reg.add_converter(Rc::clone(&c));
    assert_eq!(reg.all_converters().len(), 1);
    reg.remove_converter(Some(&c));
}

#[test]
fn model_converter_registry_no_duplicate() {
    let reg = ModelConverterRegistry::instance();
    let c = mock_converter();
    reg.add_converter(Rc::clone(&c));
    reg.add_converter(Rc::clone(&c));
    assert_eq!(reg.all_converters().len(), 1);
    reg.remove_converter(Some(&c));
}

#[test]
fn model_converter_registry_remove_null() {
    let reg = ModelConverterRegistry::instance();
    reg.remove_converter(None);
}

#[test]
fn model_converter_registry_find_by_metamodels() {
    let reg = ModelConverterRegistry::instance();
    let c = mock_converter();
    reg.add_converter(Rc::clone(&c));
    let found = reg.find_converter("src", "tgt").expect("match");
    assert!(Rc::ptr_eq(&found, &c));
    assert!(reg.find_converter("tgt", "src").is_none());
    assert!(reg.find_converter("xxx", "yyy").is_none());
    reg.remove_converter(Some(&c));
}

// ===========================================================================
// ScopingTests.cpp (10)
// ===========================================================================

#[test]
fn file_resource_scope_belongs_to_uri() {
    let scope = FileResourceScope::new(Uri::parse("file:///tmp/a.xmi"));
    assert!(scope.belongs_to_uri(&Uri::parse("file:///tmp/a.xmi"), false));
    assert!(!scope.belongs_to_uri(&Uri::parse("file:///tmp/b.xmi"), false));
    assert!(scope.did_belong_to_uri(&Uri::parse("file:///tmp/a.xmi"), false));
    assert!(!scope.did_belong_to_uri(&Uri::parse("file:///tmp/b.xmi"), false));
}

#[test]
fn file_resource_scope_root_and_persisted() {
    let scope = FileResourceScope::new(Uri::parse("file:///tmp/a.xmi"));
    assert_eq!(scope.get_root_uri().to_string(), "file:///tmp/a.xmi");
    let persisted = scope.get_persisted_files(false);
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].to_string(), "file:///tmp/a.xmi");
    assert!(scope.get_referenced_root_uris().is_empty());
    assert!(scope.get_referencing_root_uris().is_empty());
}

#[test]
fn file_resource_scope_is_shared() {
    let scope = FileResourceScope::new(Uri::parse("file:///tmp/a.xmi"));
    assert!(!scope.is_shared_uri(&Uri::parse("file:///tmp/a.xmi")));
    assert!(!scope.is_shared_resource(None));
}

#[test]
fn file_resource_scope_belongs_to_resource() {
    let scope = FileResourceScope::new(Uri::parse("file:///tmp/a.xmi"));
    let r = Resource::new(Uri::parse("file:///tmp/a.xmi"));
    assert!(scope.belongs_to_resource(Some(&r), false));
    let other = Resource::new(Uri::parse("file:///tmp/b.xmi"));
    assert!(!scope.belongs_to_resource(Some(&other), false));
    assert!(!scope.belongs_to_resource(None, false));
}

#[test]
fn file_resource_scope_provider_by_uri() {
    let p = FileResourceScopeProvider::instance();
    let scope = p
        .create_scope_from_uri(&Uri::parse("file:///tmp/a.xmi"))
        .expect("scope");
    assert_eq!(scope.get_root_uri().to_string(), "file:///tmp/a.xmi");
}

#[test]
fn file_resource_scope_provider_by_resource() {
    let p = FileResourceScopeProvider::instance();
    let r = Resource::new(Uri::parse("file:///tmp/a.xmi"));
    let scope = p.create_scope_from_resource(Some(&r)).expect("scope");
    assert_eq!(scope.get_root_uri().to_string(), "file:///tmp/a.xmi");
    assert!(p.create_scope_from_resource(None).is_none());
}

#[test]
fn file_resource_scope_provider_by_eobject_null() {
    let p = FileResourceScopeProvider::instance();
    assert!(p.create_scope_from_object(None).is_none());
}

#[test]
fn resource_scope_provider_registry_register_and_create() {
    let reg = ResourceScopeProviderRegistry::instance();
    let r = Resource::new(Uri::parse("file:///tmp/c.xmi"));
    assert!(reg.create_scope_from_resource(Some(&r)).is_none());
    assert!(reg.is_not_in_any_scope(&Uri::parse("file:///tmp/d.xmi")));
}

#[test]
fn resource_scope_provider_registry_register_provider() {
    let reg = ResourceScopeProviderRegistry::instance();
    let p: Rc<dyn ResourceScopeProvider> = Rc::new(FileResourceScopeProvider::instance());
    reg.register_provider(Rc::clone(&p));
    let r = Resource::new(Uri::parse("file:///tmp/e.xmi"));
    let scope = reg.create_scope_from_resource(Some(&r)).expect("scope");
    assert_eq!(scope.get_root_uri().to_string(), "file:///tmp/e.xmi");
    assert!(!reg.is_not_in_any_scope(&Uri::parse("file:///tmp/e.xmi")));
    reg.unregister_provider(&p);
}

#[test]
fn resource_scope_provider_registry_duplicate_register() {
    let reg = ResourceScopeProviderRegistry::instance();
    let p: Rc<dyn ResourceScopeProvider> = Rc::new(FileResourceScopeProvider::instance());
    reg.register_provider(Rc::clone(&p));
    reg.register_provider(Rc::clone(&p));
    reg.unregister_provider(&p);
    let r = Resource::new(Uri::parse("file:///tmp/f.xmi"));
    assert!(reg.create_scope_from_resource(Some(&r)).is_none());
}
