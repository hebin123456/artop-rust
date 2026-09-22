//! C++ parity suite: `XMLHelperTests.cpp` contract.
//!
//! Ports every assertion from the C++ `emf-xmi/tests/XMLHelperTests.cpp`:
//! the pure `XMIHelper` utilities (`splitQName`, `splitHref`,
//! `stripFragmentSlash`, `escapeXmlAttr`, `escapeXmlText`), the four namespace
//! URI constants, and the `XMLHelperImpl` namespace-context / encoding / URI
//! holder behavior. Aligned to Java
//! `org.eclipse.emf.ecore.xmi.impl.XMIHelperImpl` / `XMLHelperImpl`.

use emf_common::resource::Resource;
use emf_common::uri::Uri;
use emf_xmi::xml_helper::FeatureKind;
use emf_xmi::{
    escape_xml_attr, escape_xml_attr_with_limit, escape_xml_text, split_href, split_qname,
    strip_fragment_slash, XMLHelper, K_ECORE_NS_URI, K_XMI_NS_URI, K_XMI_NS_URI_2, K_XSI_NS_URI,
};

// ---------------------------------------------------------------------------
// 1) splitQName
// ---------------------------------------------------------------------------
#[test]
fn split_qname_with_prefix() {
    let (p, l) = split_qname("ecore:EClass");
    assert_eq!(p, "ecore");
    assert_eq!(l, "EClass");
}

#[test]
fn split_qname_no_colon() {
    let (p, l) = split_qname("EPackage");
    assert_eq!(p, "");
    assert_eq!(l, "EPackage");
}

#[test]
fn split_qname_empty_string() {
    let (p, l) = split_qname("");
    assert_eq!(p, "");
    assert_eq!(l, "");
}

// ---------------------------------------------------------------------------
// 2) splitHref
// ---------------------------------------------------------------------------
#[test]
fn split_href_path_and_fragment() {
    let r = split_href("library.ecore#//Library");
    assert_eq!(r.path, "library.ecore");
    assert_eq!(r.fragment, "//Library");
}

#[test]
fn split_href_only_fragment() {
    let r = split_href("#//Book");
    assert_eq!(r.path, "");
    assert_eq!(r.fragment, "//Book");
}

#[test]
fn split_href_only_path() {
    let r = split_href("other.ecore");
    assert_eq!(r.path, "other.ecore");
    assert_eq!(r.fragment, "");
}

#[test]
fn split_href_ecore_edatatype_form() {
    let r = split_href("ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString");
    assert_eq!(r.path, "http://www.eclipse.org/emf/2002/Ecore");
    assert_eq!(r.fragment, "//EString");
}

#[test]
fn split_href_ecore_eclass_form() {
    let r = split_href("ecore:EClass library.ecore#//Book");
    assert_eq!(r.path, "library.ecore");
    assert_eq!(r.fragment, "//Book");
}

// ---------------------------------------------------------------------------
// 3) stripFragmentSlash
// ---------------------------------------------------------------------------
#[test]
fn strip_fragment_double_slash() {
    assert_eq!(strip_fragment_slash("//EString"), "EString");
}

#[test]
fn strip_fragment_single_slash() {
    assert_eq!(strip_fragment_slash("/Library"), "Library");
}

#[test]
fn strip_fragment_no_slash() {
    assert_eq!(strip_fragment_slash("Book"), "Book");
}

#[test]
fn strip_fragment_path_style() {
    assert_eq!(strip_fragment_slash("//Container/feature.name"), "Container/feature.name");
}

#[test]
fn strip_fragment_empty() {
    assert_eq!(strip_fragment_slash(""), "");
}

// ---------------------------------------------------------------------------
// 4) escapeXmlAttr
// ---------------------------------------------------------------------------
#[test]
fn escape_attr_basic_chars() {
    assert_eq!(escape_xml_attr("a&b"), "a&amp;b");
    assert_eq!(escape_xml_attr("a<b"), "a&lt;b");
    assert_eq!(escape_xml_attr("a\"b"), "a&quot;b");
    // Java attribute values leave > and ' unescaped.
    assert_eq!(escape_xml_attr("a>b"), "a>b");
    assert_eq!(escape_xml_attr("a'b"), "a'b");
}

#[test]
fn escape_attr_control_chars() {
    assert_eq!(escape_xml_attr("a\nb"), "a&#xA;b");
    assert_eq!(escape_xml_attr("a\rb"), "a&#xD;b");
    assert_eq!(escape_xml_attr("a\tb"), "a&#x9;b");
}

#[test]
fn escape_attr_no_special_chars() {
    assert_eq!(escape_xml_attr("hello world 123"), "hello world 123");
}

#[test]
fn escape_attr_all_specials() {
    assert_eq!(escape_xml_attr("<&>\"'"), "&lt;&amp;>&quot;'");
}

#[test]
fn escape_attr_non_ascii() {
    assert_eq!(escape_xml_attr("a中b"), "a&#x4e2d;b"); // 中
    assert_eq!(escape_xml_attr("aéb"), "a&#xe9;b"); // é
}

#[test]
fn escape_attr_utf8_passthrough() {
    // encoding=UTF-8 (mappableLimit=0x10FFFF) -> non-ASCII passes through.
    assert_eq!(escape_xml_attr_with_limit("a中b", 0x10FFFF), "a中b");
}

#[test]
fn escape_attr_empty() {
    assert_eq!(escape_xml_attr(""), "");
}

// ---------------------------------------------------------------------------
// 5) escapeXmlText
// ---------------------------------------------------------------------------
#[test]
fn escape_text_basic_chars() {
    assert_eq!(escape_xml_text("a&b"), "a&amp;b");
    assert_eq!(escape_xml_text("a<b"), "a&lt;b");
    assert_eq!(escape_xml_text("a\"b"), "a&quot;b");
    // convertText leaves > and ' unescaped.
    assert_eq!(escape_xml_text("a>b"), "a>b");
    assert_eq!(escape_xml_text("a'b"), "a'b");
}

#[test]
fn escape_text_keeps_newline_tab() {
    assert_eq!(escape_xml_text("a\nb"), "a\nb");
    assert_eq!(escape_xml_text("a\tb"), "a\tb");
    // convertText escapes \r.
    assert_eq!(escape_xml_text("a\rb"), "a&#xD;b");
}

#[test]
fn escape_text_no_special_chars() {
    assert_eq!(escape_xml_text("plain text 42"), "plain text 42");
}

// ---------------------------------------------------------------------------
// 6) namespace URI constants
// ---------------------------------------------------------------------------
#[test]
fn namespace_constants() {
    assert_eq!(K_ECORE_NS_URI, "http://www.eclipse.org/emf/2002/Ecore");
    assert_eq!(K_XMI_NS_URI, "http://www.omg.org/XMI");
    assert_eq!(K_XMI_NS_URI_2, "http://schema.omg.org/spec/XMI/2.0");
    assert_eq!(K_XSI_NS_URI, "http://www.w3.org/2001/XMLSchema-instance");
}

// ---------------------------------------------------------------------------
// 7) XMLHelperImpl: namespace context add/get
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_namespace_context_add_and_get() {
    let mut h = XMLHelper::new();
    h.push_context();
    h.add_prefix("ecore", K_ECORE_NS_URI);
    h.add_prefix("xmi", K_XMI_NS_URI);
    assert_eq!(h.get_uri("ecore"), Some(K_ECORE_NS_URI));
    assert_eq!(h.get_uri("xmi"), Some(K_XMI_NS_URI));
    assert_eq!(h.get_prefix(K_ECORE_NS_URI), Some("ecore"));
    assert_eq!(h.get_prefix(K_XMI_NS_URI), Some("xmi"));
    assert_eq!(h.get_namespace_uri("ecore"), Some(K_ECORE_NS_URI));
    h.pop_context();
}

// ---------------------------------------------------------------------------
// 8) pop clears
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_namespace_context_pop_clears() {
    let mut h = XMLHelper::new();
    h.push_context();
    h.add_prefix("ec", K_ECORE_NS_URI);
    assert_eq!(h.get_uri("ec"), Some(K_ECORE_NS_URI));
    h.pop_context();
    // After popping, the prefix is no longer visible (C++ returns "").
    assert_eq!(h.get_uri("ec").map(|s| s.to_string()).unwrap_or_default(), "");
}

// ---------------------------------------------------------------------------
// 9) nested contexts
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_namespace_context_nested() {
    let mut h = XMLHelper::new();
    h.push_context();
    h.add_prefix("a", "urn:a");
    assert_eq!(h.get_uri("a"), Some("urn:a"));
    h.push_context();
    h.add_prefix("b", "urn:b");
    assert_eq!(h.get_uri("a"), Some("urn:a"));
    assert_eq!(h.get_uri("b"), Some("urn:b"));
    h.pop_context();
    // Inner-before-outer pop: b invisible, a still visible.
    assert_eq!(h.get_uri("b").map(|s| s.to_string()).unwrap_or_default(), "");
    assert_eq!(h.get_uri("a"), Some("urn:a"));
    h.pop_context();
}

// ---------------------------------------------------------------------------
// 10) encoding mapping
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_encoding_mapping() {
    let h = XMLHelper::new();
    assert!(!h.get_xml_encoding("ASCII").is_empty());
    assert!(!h.get_java_encoding("UTF-8").is_empty());
}

// ---------------------------------------------------------------------------
// 11) resource holder (set/get)
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_resource_setter_getter() {
    let mut h = XMLHelper::new();
    assert!(h.resource().is_none());
    h.set_resource(None);
    assert!(h.resource().is_none());

    // set/get a concrete resource round-trips (C++ null-contract plus a
    // non-null sanity check).
    let res = Resource::new(Uri::parse("file:/tmp/library.ecore"));
    h.set_resource(Some(res));
    assert!(h.resource().is_some());
}

// ---------------------------------------------------------------------------
// 12) no-namespace package
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_no_namespace_package() {
    let mut h = XMLHelper::new();
    assert!(h.no_namespace_package().is_none());
    h.set_no_namespace_package("gen");
    assert_eq!(h.no_namespace_package(), Some("gen"));
}

// ---------------------------------------------------------------------------
// 13) feature kind constants
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_feature_kind_constants() {
    assert_eq!(FeatureKind::DatatypeSingle.code(), 1);
    assert_eq!(FeatureKind::DatatypeMany.code(), 2);
    assert_eq!(FeatureKind::IsManyAdd.code(), 3);
    assert_eq!(FeatureKind::IsManyMove.code(), 4);
    assert_eq!(FeatureKind::Other.code(), 5);
}

// ---------------------------------------------------------------------------
// 14) base URI setter/getter
// ---------------------------------------------------------------------------
#[test]
fn xml_helper_base_uri() {
    let mut h = XMLHelper::new();
    let u = Uri::create_file_uri("/tmp/library.ecore");
    h.set_base_uri(Some(u));
    let got = h.base_uri().expect("base URI set");
    assert_eq!(got.to_file_path(), "/tmp/library.ecore");
}