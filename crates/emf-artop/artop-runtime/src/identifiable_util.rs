//! `IdentifiableUtil` — reflective access to the AUTOSAR `Identifiable` surface.
//!
//! Port of C++ `emf::artop::runtime::IdentifiableUtil` (aligned to Java
//! `org.artop.aal.common.util.IdentifiableUtil`).
//!
//! Almost every "named" object in an AUTOSAR model implements `Identifiable`.
//! This helper reads/writes `shortName` / `longName` / `description` / `identifier`
//! / `uuid` through the `EClass`'s `eAllStructuralFeatures` reflection, so it is
//! independent of the concrete metamodel (dynamic `.ecore` or the generated
//! static registry).
//!
//! C++ models a `nullptr` object; the Rust port uses `Option<&ObjectRef>` so the
//! same null-safety contract is expressible (`IdentifiableUtil::get_short_name(None)`
//! is empty, `set_short_name(None, ..)` is a no-op).

use emf_common::value::{ObjectRef, Val};

/// AUTOSAR `Identifiable` reflective accessors (C++ `IdentifiableUtil`).
pub struct IdentifiableUtil;

impl IdentifiableUtil {
    /// Common identifier feature name.
    pub const SHORT_NAME_FEATURE: &'static str = "shortName";
    /// Long-name feature name (multi-valued reference).
    pub const LONG_NAME_FEATURE: &'static str = "longName";
    /// Description feature name (multi-valued reference).
    pub const DESCRIPTION_FEATURE: &'static str = "description";
    /// Identifier feature name.
    pub const IDENTIFIER_FEATURE: &'static str = "identifier";
    /// UUID feature name.
    pub const UUID_FEATURE: &'static str = "uuid";

    /// Whether `obj` declares a `shortName` feature.
    pub fn has_short_name(obj: Option<&ObjectRef>) -> bool {
        has_feature(obj, Self::SHORT_NAME_FEATURE)
    }

    /// Read `shortName` (empty when the object is absent or unset).
    pub fn get_short_name(obj: Option<&ObjectRef>) -> String {
        read_string(obj, Self::SHORT_NAME_FEATURE)
    }

    /// Write `shortName` (no-op when the object is absent or has no such feature).
    pub fn set_short_name(obj: Option<&ObjectRef>, name: &str) {
        write_string(obj, Self::SHORT_NAME_FEATURE, name);
    }

    /// Read `longName`.
    ///
    /// Mirrors the C++ simplification: `longName` is a multi-valued reference to
    /// `MULTILANGUAGE-LONG-NAME` entries, and resolving the language-tagged text
    /// is left to codegen, so this returns an empty string.
    pub fn get_long_name(obj: Option<&ObjectRef>) -> String {
        // Faithful to C++: presence is checked, the multi-valued structure is not
        // flattened here.
        let _ = has_feature(obj, Self::LONG_NAME_FEATURE);
        String::new()
    }

    /// Write `longName` (C++ placeholder: no-op).
    pub fn set_long_name(_obj: Option<&ObjectRef>, _name: &str) {}

    /// Read `description` (C++ simplification: multi-valued, returns empty).
    pub fn get_description(obj: Option<&ObjectRef>) -> String {
        let _ = has_feature(obj, Self::DESCRIPTION_FEATURE);
        String::new()
    }

    /// Write `description` (C++ placeholder: no-op).
    pub fn set_description(_obj: Option<&ObjectRef>, _desc: &str) {}

    /// Read `identifier` (empty when absent/unset).
    pub fn get_identifier(obj: Option<&ObjectRef>) -> String {
        read_string(obj, Self::IDENTIFIER_FEATURE)
    }

    /// Write `identifier` (no-op when absent).
    pub fn set_identifier(obj: Option<&ObjectRef>, id: &str) {
        write_string(obj, Self::IDENTIFIER_FEATURE, id);
    }

    /// Read `uuid` (aligned to Java `IdentifiableUtil.getUUID`).
    pub fn get_uuid(obj: Option<&ObjectRef>) -> String {
        read_string(obj, Self::UUID_FEATURE)
    }

    /// Build an identity-provider callback matching the Java ARTOP semantics:
    /// `shortName` first (unique among same-typed siblings), then `uuid`
    /// (globally unique). The returned closure has the same shape as
    /// `emf_compare::IdentifierProvider` (`Fn(&ObjectRef) -> String`), so the
    /// artop layer need not depend on `emf-compare` headers.
    pub fn as_identifier_provider() -> Box<dyn Fn(&ObjectRef) -> String> {
        Box::new(|obj: &ObjectRef| {
            let sn = Self::get_short_name(Some(obj));
            if !sn.is_empty() {
                return sn;
            }
            Self::get_uuid(Some(obj))
        })
    }
}

/// Whether `obj`'s class declares a feature named `name`.
///
/// `EObject::e_get` returns `Some(_)` (possibly the class default) exactly when
/// the feature belongs to `eAllStructuralFeatures`, so a `Some` result is the
/// reflective "feature exists" answer.
fn has_feature(obj: Option<&ObjectRef>, name: &str) -> bool {
    match obj {
        Some(o) => o.borrow().e_get(name).is_some(),
        None => false,
    }
}

/// Read a string-valued feature, returning empty when absent or non-string.
fn read_string(obj: Option<&ObjectRef>, name: &str) -> String {
    let Some(o) = obj else {
        return String::new();
    };
    let o = o.borrow();
    match o.e_get(name) {
        Some(Val::String(s)) => s,
        _ => String::new(),
    }
}

/// Write a string-valued feature (no-op when the feature does not exist).
fn write_string(obj: Option<&ObjectRef>, name: &str, value: &str) {
    let Some(o) = obj else {
        return;
    };
    let mut o = o.borrow_mut();
    let _ = o.e_set(name, Val::String(value.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::value::ObjectRef;
    use emf_ecore::{
        make_package_ref, DynamicEObject, EClass, EClassKind, EStructuralFeature, PackageRegistry,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A `Referrable`-like class with `shortName` / `uuid` / `identifier`.
    fn referrable_registry() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("autosar40");
        pkg.set_ns_uri("http://autosar.org/schema/r4.0");
        let mut cls = EClass::new("Referrable", EClassKind::Class);
        for name in ["shortName", "uuid", "identifier"] {
            let mut f = EStructuralFeature::attribute(name);
            f.set_type_name("EString");
            cls.add_feature(f);
        }
        pkg.add_class(cls);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    fn new_referrable(reg: &PackageRegistry) -> ObjectRef {
        let class = reg.find_class("Referrable").expect("Referrable registered");
        Rc::new(RefCell::new(DynamicEObject::new_in(class, reg.clone())))
    }

    #[test]
    fn null_object_is_safe() {
        // Mirrors C++ `test_identifiable_util_null`.
        assert_eq!(IdentifiableUtil::get_short_name(None), "");
        assert!(!IdentifiableUtil::has_short_name(None));
        IdentifiableUtil::set_short_name(None, "x"); // must not panic
    }

    #[test]
    fn short_name_round_trips() {
        let reg = referrable_registry();
        let obj = new_referrable(&reg);
        assert!(IdentifiableUtil::has_short_name(Some(&obj)));
        IdentifiableUtil::set_short_name(Some(&obj), "abc");
        assert_eq!(IdentifiableUtil::get_short_name(Some(&obj)), "abc");
        assert_eq!(
            obj.borrow().e_get("shortName"),
            Some(Val::String("abc".into()))
        );
    }

    #[test]
    fn identifier_provider_prefers_short_name_then_uuid() {
        let reg = referrable_registry();
        let obj = new_referrable(&reg);
        let provider = IdentifiableUtil::as_identifier_provider();
        // Nothing set: empty id.
        assert_eq!(provider(&obj), "");
        IdentifiableUtil::set_identifier(Some(&obj), "ID-1");
        // identifier is not consulted by the provider; uuid is.
        assert_eq!(provider(&obj), "");
        IdentifiableUtil::set_short_name(Some(&obj), "SN");
        assert_eq!(provider(&obj), "SN");
        obj.borrow_mut().e_set("uuid", Val::String("UUID-9".into()));
        // shortName still wins.
        assert_eq!(provider(&obj), "SN");
        obj.borrow_mut().e_unset("shortName");
        assert_eq!(provider(&obj), "UUID-9");
    }

    #[test]
    fn unknown_feature_is_absent() {
        let reg = referrable_registry();
        let obj = new_referrable(&reg);
        // longName / description are not declared on this class.
        assert!(!has_feature(
            Some(&obj),
            IdentifiableUtil::LONG_NAME_FEATURE
        ));
        assert_eq!(IdentifiableUtil::get_long_name(Some(&obj)), "");
    }
}
