//! XSD metamodel (port of C++ `emf-xsd`).
//!
//! Port target: C++ `emf-xsd` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod xsd_metamodel {
    //! Port target: C++ source unit for `xsd_metamodel`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xsd::xsd_metamodel"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::xsd_metamodel::api_surface(),
            "emf-xsd::xsd_metamodel"
        );
    }
}
