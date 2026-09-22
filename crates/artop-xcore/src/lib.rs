//! Xcore DSL parser (port of C++ `emf-xcore`).
//!
//! Port target: C++ `emf-xcore` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod parser {
    //! Port target: C++ source unit for `parser`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-xcore::parser"
    }
}

pub mod dsl {
    //! Port target: C++ source unit for `dsl`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "artop-xcore::dsl"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(super::parser::api_surface(), "artop-xcore::parser");
    }
}
