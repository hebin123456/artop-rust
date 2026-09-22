//! Generate static models from .ecore (port of C++ `emf-artop/emf-artop-codegen`).
//!
//! Port target: C++ `emf-codegen` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod generator {
    //! Port target: C++ source unit for `generator`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-codegen::generator"
    }
}

pub mod ecore_to_model {
    //! Port target: C++ source unit for `ecore_to_model`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-codegen::ecore_to_model"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(super::generator::api_surface(), "artop-codegen::generator");
    }
}
