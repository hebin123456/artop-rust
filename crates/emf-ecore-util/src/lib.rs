//! Extra Ecore utility layer (port of C++ `emf-ecore-util`).
//!
//! Port target: C++ `emf-ecore-util` module of `hebin123456/artop-cpp`
//! (aligned to Java `org.eclipse.emf.ecore.util`).
//!
//! Generic EMF utilities over `DynamicEObject` reflection — no domain
//! metamodel (e.g. AUTOSAR) knowledge is allowed here; see the decoupling
//! principle in `docs/PROGRESS.md`.

/// `EcoreUtil`: containment-tree and cross-reference helpers.
pub mod ecore_util;

/// `EcoreUtil.Copier`: deep copy of an `EObject` graph.
pub mod copier;

/// `EObjectValidator`: structural validation of `EPackage` metadata.
pub mod e_object_validator;

/// `FeatureMap` / `BasicFeatureMap`: ordered `(feature, value)` entry list.
pub mod feature_map;

/// `ECrossReferenceAdapter`: collect non-containment references of a subtree.
pub mod e_cross_reference_adapter;

/// `EcoreSwitch`: visitor dispatching on the Ecore meta-class of an object.
pub mod ecore_switch;

pub mod extended_metadata {
    //! Port target: C++ source unit for `extended_metadata`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::extended_metadata"
    }
}

pub mod conversion_delegate {
    //! Port target: C++ source unit for `conversion_delegate`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::conversion_delegate"
    }
}

pub mod e_contents_elist {
    //! Port target: C++ source unit for `e_contents_elist`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::e_contents_elist"
    }
}

pub mod eobject_containment_elist {
    //! Port target: C++ source unit for `eobject_containment_elist`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::eobject_containment_elist"
    }
}

pub mod eobject_containment_inverse_elist {
    //! Port target: C++ source unit for `eobject_containment_inverse_elist`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::eobject_containment_inverse_elist"
    }
}

pub mod e_object_elist;

/// `EobjectResolvingEList`: EObject list resolving proxies lazily.
pub mod eobject_resolving_elist {
    //! Port target: C++ source unit for `eobject_resolving_elist`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::eobject_resolving_elist"
    }
}

pub mod eobject_inverse_elist {
    //! Port target: C++ source unit for `eobject_inverse_elist`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::eobject_inverse_elist"
    }
}

pub mod eobject_inverse_resolving_elist {
    //! Port target: C++ source unit for `eobject_inverse_resolving_elist`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::eobject_inverse_resolving_elist"
    }
}

pub mod validator_registry {
    //! Port target: C++ source unit for `validator_registry`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::validator_registry"
    }
}

pub mod ecore_adapter_factory {
    //! Port target: C++ source unit for `ecore_adapter_factory`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::ecore_adapter_factory"
    }
}

pub mod ecore_emap {
    //! Port target: C++ source unit for `ecore_emap`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::ecore_emap"
    }
}

pub mod ecore_validator;

pub mod equality_helper;

pub mod feature_map_util {
    //! Port target: C++ source unit for `feature_map_util`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-util::feature_map_util"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn created_real_modules_exist() {
        // Smoke: the real utility modules are wired in and compile as pub.
        let _ = super::ecore_util::ptr;
        let _ = super::copier::Copier::new();
    }
}
