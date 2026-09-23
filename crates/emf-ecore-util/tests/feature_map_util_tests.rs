//! Rust port parity tests for `FeatureMapUtil` (C++ `emf-ecore-util`, aligned
//! to Java `org.eclipse.emf.ecore.util.FeatureMapUtil`).
//!
//! No dedicated C++ test file exists for this unit; these tests lock the
//! ported semantics: wildcard/group/anyAttribute/featureMap predicates, many,
//! document-root detection, entry creation, FeatureMap accessors, and the
//! `ns#name` QName decoders.

use emf_common::value::Val;
use emf_ecore::{EClass, EClassKind, EStructuralFeature};
use emf_ecore_util::feature_map::{FeatureMap};
use emf_ecore_util::feature_map_util::FeatureMapUtil;

fn attr(name: &str, upper: i32) -> EStructuralFeature {
    let mut f = EStructuralFeature::attribute(name);
    f.set_upper_bound(upper);
    f
}

fn attr_typed(name: &str, upper: i32, ty: &str) -> EStructuralFeature {
    let mut f = attr(name, upper);
    f.set_type_name(ty);
    f
}

// ===== 基本谓词 =====
#[test]
fn is_wildcard_true_for_star() {
    assert!(FeatureMapUtil::is_wildcard(&attr("*", -1)));
    assert!(FeatureMapUtil::is_wildcard(&attr("*", 2)));
}

#[test]
fn is_wildcard_true_for_colon_prefix() {
    assert!(FeatureMapUtil::is_wildcard(&attr(":directive", -1)));
}

#[test]
fn is_wildcard_false_for_plain_or_single() {
    assert!(!FeatureMapUtil::is_wildcard(&attr("name", -1)));
    assert!(!FeatureMapUtil::is_wildcard(&attr("name", 1))); // single-valued
}

#[test]
fn is_many_true_when_unbounded() {
    assert!(FeatureMapUtil::is_many(&attr("x", -1)));
    assert!(FeatureMapUtil::is_many(&attr("x", 3)));
    assert!(!FeatureMapUtil::is_many(&attr("x", 1)));
}

#[test]
fn is_group_suffix() {
    assert!(FeatureMapUtil::is_group(&attr("group", -1)));
    assert!(FeatureMapUtil::is_group(&attr("sub:group", -1)));
    assert!(!FeatureMapUtil::is_group(&attr("other", -1)));
    assert!(!FeatureMapUtil::is_group(&attr("group", 1)));
}

#[test]
fn is_any_attribute_by_entry_type() {
    let f = attr_typed("any", -1, "org.eclipse.emf.ecore.util.FeatureMap$Entry");
    assert!(FeatureMapUtil::is_any_attribute(&f));
    assert!(!FeatureMapUtil::is_any_attribute(&attr("a", -1)));
}

#[test]
fn is_feature_map_by_type() {
    let f = attr_typed("fm", -1, "org.eclipse.emf.ecore.util.FeatureMap");
    assert!(FeatureMapUtil::is_feature_map(&f));
    assert!(!FeatureMapUtil::is_feature_map(&attr("a", -1)));
}

#[test]
fn is_document_root_by_class_name() {
    let dr = EClass::new("DocumentRoot", EClassKind::Class);
    let other = EClass::new("Foo", EClassKind::Class);
    assert!(FeatureMapUtil::is_document_root(&dr));
    assert!(!FeatureMapUtil::is_document_root(&other));
}

// ===== 入口创建与 FeatureMap 访问 =====
#[test]
fn create_entry_binds_feature_and_value() {
    let f = attr("a", -1);
    let entry = FeatureMapUtil::create_entry(f.clone(), Val::Int(7));
    assert_eq!(entry.feature().name(), "a");
    assert_eq!(entry.value().as_int(), Some(7));
}

#[test]
fn entries_and_values_per_feature() {
    let mut a = attr("a", -1);
    let mut b = attr("b", -1);
    a.set_feature_id(0);
    b.set_feature_id(1);
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), Val::Int(1));
    fm.add_entry(b.clone(), Val::Int(2));
    fm.add_entry(a.clone(), Val::Int(3));

    assert_eq!(FeatureMapUtil::size(&fm, &a), 2);
    assert_eq!(FeatureMapUtil::size(&fm, &b), 1);
    assert!(!FeatureMapUtil::is_empty(&fm, &a));
    assert!(FeatureMapUtil::is_empty(&fm, &attr("zz", -1)));

    let vals = FeatureMapUtil::values(&fm, &a);
    assert_eq!(vals.len(), 2);
    assert_eq!(vals[0].as_int(), Some(1));
    assert_eq!(vals[1].as_int(), Some(3));

    let es = FeatureMapUtil::entries(&fm, &b);
    assert_eq!(es.len(), 1);
    assert_eq!(es[0].value().as_int(), Some(2));
}

#[test]
fn has_entries_detects_presence() {
    let mut a = attr("a", -1);
    a.set_feature_id(0);
    let mut fm = FeatureMap::new();
    assert!(!FeatureMapUtil::has_entries(&fm, &a));
    fm.add_entry(a.clone(), Val::Int(1));
    assert!(FeatureMapUtil::has_entries(&fm, &a));
}

// ===== QName / 名称拆分 =====
#[test]
fn decode_feature_name_hash() {
    let (ns, name) = FeatureMapUtil::decode_feature_name("http://x/y#foo");
    assert_eq!(ns, "http://x/y");
    assert_eq!(name, "foo");
}

#[test]
fn decode_feature_name_unqualified() {
    let (ns, name) = FeatureMapUtil::decode_feature_name("plain");
    assert_eq!(ns, "");
    assert_eq!(name, "plain");
}

#[test]
fn split_name_matches_decode() {
    assert_eq!(
        FeatureMapUtil::split_name("a#b"),
        FeatureMapUtil::decode_feature_name("a#b")
    );
}