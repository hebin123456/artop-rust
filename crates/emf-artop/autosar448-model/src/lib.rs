//! # autosar448-model
//!
//! A *generated* registry of the AUTOSAR metamodel, produced from the `.ecore`
//! sources in `hebin123456/artop-cpp`:
//!
//! * `models/gautosar/gautosar.ecore` — the generic AUTOSAR layer
//!   (`GARObject`, `GReferrable`, `GIdentifiable`, ...).
//! * `models/autosar448/autosar448.ecore` — the full AUTOSAR 4.4.8 model whose
//!   base classes (`ARObject`, `Referrable`, ...) inherit the generic layer.
//!
//! Both files are merged into a single flat registry so that inheritance and
//! reflection are *plain data*: every `EClass` (name, `eSuperTypes`, own
//! features) and every `EStructuralFeature` (kind, `eType`, containment,
//! multiplicity, and the AUTOSAR serialization metadata `xml.name`,
//! `xml.namePlural`, `internal-xml-sequenceOffset`, APRXML role/type element
//! flags) is emitted as a `static` table.
//!
//! Because inheritance is data, the 2105-class, 6122-feature metamodel compiles
//! in seconds. [`reflect`] implements the EMF algorithms
//! (`eAllFeatures` / `isSuperTypeOf` / `eGet`) plus the `xml.name` ↔ feature
//! lookups the arxml layer needs.
//!
//! Regenerate with
//! `tools/gen-autosar448-model.py <gautosar.ecore> <autosar448.ecore> <out.rs>`.

pub mod registry;

/// EMF reflection algorithms over the registry (`eAllFeatures`, `isSuperTypeOf`,
/// `eGet`, `xml.name` lookups), plus the reflective `RObject` / `Val` model.
pub mod reflect;

/// Bridge from the static registry to the generic EMF metamodel: builds and
/// registers the merged AUTOSAR `EPackage` so `DynamicEObject` / the XMI
/// loader/saver and the arxml layer can instantiate AUTOSAR objects.
pub mod metamodel;

pub use metamodel::{
    build_autosar_package, register_autosar_metamodel, AUTOSAR_BASE_NS_URI, AUTOSAR_PACKAGE_NAME,
};

pub use reflect::{
    all_feature_ids, all_super_ids, class_id, class_id_by_xml_name, feature, find_feature,
    find_feature_by_xml, is_super_type_of, reference_target, FeatureId, RObject, Val,
};
pub use registry::{
    ClassMeta, DataTypeMeta, EnumMeta, FeatureKind, FeatureMeta, TypeRef, ECLASS, FEATURE_META,
    N_CLASSES, N_DATATYPES, N_ENUMS, N_FEATURES, N_PACKAGES,
};

/// Number of registered EClasses (gautosar + autosar448 merged).
pub const CLASS_COUNT: usize = N_CLASSES;

#[cfg(test)]
mod tests {
    use super::reflect::*;
    use super::*;

    #[test]
    fn registered_scale() {
        // Regression guard on generation (gautosar 180 + autosar448 1925).
        assert_eq!(N_CLASSES, 2105);
        assert_eq!(N_FEATURES, 6122);
        assert_eq!(N_ENUMS, 291);
        assert_eq!(N_DATATYPES, 67);
    }

    #[test]
    fn generated_metadata_is_consistent() {
        for (i, c) in ECLASS.iter().enumerate() {
            for &s in c.sups {
                assert!((s as usize) < N_CLASSES, "class {i} bad super {s}");
            }
            for &f in c.own {
                assert!((f as usize) < N_FEATURES, "class {i} bad feature {f}");
            }
        }
    }

    #[test]
    fn name_and_xml_lookup_agree() {
        for name in [
            "AUTOSAR",
            "ARPackage",
            "Referrable",
            "GARObject",
            "SwcImplementation",
        ] {
            assert!(class_id(name).is_some(), "missing class {name}");
        }
        // ARObject's arxml element name is AROBJECT.
        assert_eq!(class_id_by_xml_name("AR-PACKAGE"), class_id("ARPackage"));
    }

    #[test]
    fn supertype_relation_holds() {
        let swc = class_id("SwcImplementation").unwrap();
        assert!(is_super_type_of(class_id("Referrable").unwrap(), swc));
        assert!(is_super_type_of(class_id("ARObject").unwrap(), swc));
        assert!(is_super_type_of(class_id("GARObject").unwrap(), swc));
    }

    #[test]
    fn arxml_metadata_is_present() {
        // AUTOSAR root class carries the multi-valued AR-PACKAGE containment.
        let autosar = class_id("AUTOSAR").unwrap();
        assert_eq!(class(autosar).xml_name, "AUTOSAR");
        let arpkg = find_feature_by_xml(autosar, "AR-PACKAGE").unwrap();
        let f = feature(arpkg);
        assert_eq!(f.name, "arPackages");
        assert!(f.containment);
        assert_eq!(f.upper, -1);
        // SHORT-NAME is a mandatory single-valued attribute on Referrable.
        let referrable = class_id("Referrable").unwrap();
        let sn = find_feature_by_xml(referrable, "SHORT-NAME").unwrap();
        let fsn = feature(sn);
        assert_eq!(fsn.kind, FeatureKind::Attribute);
        assert_eq!(fsn.lower, 1);
        assert_eq!(fsn.seq_offset, -100);
    }
}
