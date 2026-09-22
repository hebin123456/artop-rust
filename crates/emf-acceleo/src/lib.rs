//! Acceleo MTL templates / M2T engine (port of C++ `emf-acceleo`).
//!
//! Port target: C++ `emf-acceleo` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod mtl_parser {
    //! Port target: C++ source unit for `mtl_parser`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-acceleo::mtl_parser"
    }
}

pub mod template {
    //! Port target: C++ source unit for `template`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-acceleo::template"
    }
}

pub mod m2t_engine {
    //! Port target: C++ source unit for `m2t_engine`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-acceleo::m2t_engine"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(super::mtl_parser::api_surface(), "emf-acceleo::mtl_parser");
    }
}
