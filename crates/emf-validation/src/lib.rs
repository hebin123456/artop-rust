//! Model validation, batch + live (port of C++ `emf-validation`).
//!
//! Port target: C++ `emf-validation` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod annotation_constraint_loader {
    //! Port target: C++ source unit for `annotation_constraint_loader`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::annotation_constraint_loader"
    }
}

pub mod autosar_constraints {
    //! Port target: C++ source unit for `autosar_constraints`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::autosar_constraints"
    }
}

pub mod constraint_descriptor {
    //! Port target: C++ source unit for `constraint_descriptor`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::constraint_descriptor"
    }
}

pub mod constraint_parser {
    //! Port target: C++ source unit for `constraint_parser`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::constraint_parser"
    }
}

pub mod diagnostician {
    //! Port target: C++ source unit for `diagnostician`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::diagnostician"
    }
}

pub mod e_validator {
    //! Port target: C++ source unit for `e_validator`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::e_validator"
    }
}

pub mod live_validator {
    //! Port target: C++ source unit for `live_validator`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::live_validator"
    }
}

pub mod validation_service {
    //! Port target: C++ source unit for `validation_service`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::validation_service"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::annotation_constraint_loader::api_surface(),
            "emf-validation::annotation_constraint_loader"
        );
    }
}
