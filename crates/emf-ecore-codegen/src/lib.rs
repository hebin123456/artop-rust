//! GenModel-driven code generator (port of C++ `emf-ecore-codegen`).
//!
//! Port target: C++ `emf-ecore-codegen` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod genmodel {
    //! Port target: C++ source unit for `genmodel`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-codegen::genmodel"
    }
}

pub mod generator {
    //! Port target: C++ source unit for `generator`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-codegen::generator"
    }
}

pub mod templates {
    //! Port target: C++ source unit for `templates`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-ecore-codegen::templates"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::genmodel::api_surface(),
            "emf-ecore-codegen::genmodel"
        );
    }
}
