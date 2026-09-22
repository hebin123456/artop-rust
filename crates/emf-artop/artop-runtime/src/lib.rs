//! AUTOSAR serialization/deserialization, resource/factory/version metadata (port of C++ `emf-artop/emf-artop-runtime`).
//!
//! Port target: C++ `emf-runtime` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod serialization {
    //! Port target: C++ source unit for `serialization`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-runtime::serialization"
    }
}

pub mod deserialization {
    //! Port target: C++ source unit for `deserialization`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-runtime::deserialization"
    }
}

pub mod resource_factory {
    //! Port target: C++ source unit for `resource_factory`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-runtime::resource_factory"
    }
}

pub mod version_metadata {
    //! Port target: C++ source unit for `version_metadata`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-runtime::version_metadata"
    }
}

pub mod autosar_metamodel {
    //! Port target: C++ source unit for `autosar_metamodel`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-runtime::autosar_metamodel"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::serialization::api_surface(),
            "artop-runtime::serialization"
        );
    }
}
