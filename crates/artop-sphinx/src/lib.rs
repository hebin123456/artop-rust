//! Headless core subset (port of C++ `emf-sphinx`).
//!
//! Port target: C++ `emf-sphinx` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod headless_core {
    //! Port target: C++ source unit for `headless_core`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-sphinx::headless_core"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::headless_core::api_surface(),
            "artop-sphinx::headless_core"
        );
    }
}
