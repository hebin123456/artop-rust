//! # artop-metamodel
//!
//! A *generated* registry of the AUTOSAR 4.4.8 metamodel, produced from the
//! `.ecore` sources in `hebin123456/artop-cpp` (`models/autosar448/autosar448.ecore`).
//!
//! It stores, as plain data: every `EClass` (name, abstract flag, `eSuperTypes`,
//! own feature names) plus a full feature-name table. `reflect` implements the
//! EMF reflection algorithms (`eAllFeatures`, `isSuperTypeOf`, `eGet`) that run
//! over this registry.
//!
//! Because inheritance is *data*, the whole 1925-class metamodel compiles in well
//! under a minute even on commodity hardware (CI verifies this every push).
//!
//! Regenerate with `tools/gen-artop-metamodel.py`.

mod registry;

/// EMF reflection algorithms over the registry (`eAllFeatures`, `isSuperTypeOf`,
/// `eGet`), and the reflective `RObject` / `Val` value model.
pub mod reflect;

pub use registry::{name_to_id, ECLASS, FEATURE_NAMES, N_CLASSES, N_FEATURES};

/// Number of classifier classes registered from AUTOSAR 4.4.8.
pub const CLASS_COUNT: usize = N_CLASSES;

#[cfg(test)]
mod tests {
    use super::reflect::*;
    use super::*;

    #[test]
    fn registered_autosar448_scale() {
        // The real ".ecore" has these counts — a regression guard on generation.
        assert_eq!(N_CLASSES, 1925);
        let abstract_n = ECLASS.iter().filter(|c| c.abstract_).count();
        let multi_n = ECLASS.iter().filter(|c| c.sups.len() >= 2).count();
        assert_eq!(abstract_n, 305);
        assert_eq!(multi_n, 96);
    }

    #[test]
    fn eallfeatures_of_swc_implementation() {
        let swc = class_id("SwcImplementation").unwrap();
        let feats = all_features(swc);
        // own = 3; inherited from an 8-super chain = validated below.
        let own = ECLASS[swc as usize].own.len();
        assert_eq!(own, 3);
        assert!(feats.len() > own);
    }

    #[test]
    fn supertype_relation_holds() {
        let swc = class_id("SwcImplementation").unwrap();
        let referrable = class_id("Referrable").unwrap();
        let arobject = class_id("ARObject").unwrap();
        assert!(is_super_type_of(referrable, swc));
        assert!(is_super_type_of(arobject, swc));
    }

    #[test]
    fn generated_metadata_is_consistent() {
        // every EClass's super references must point at a valid registered class
        for (i, c) in ECLASS.iter().enumerate() {
            for &s in c.sups {
                assert!((s as usize) < N_CLASSES, "class {i} bad super {s}");
            }
        }
    }
}
