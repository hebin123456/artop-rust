//! Model comparison (match+diff) (port of C++ `emf-compare`).
//!
//! Port target: C++ `emf-compare` module of `hebin123456/artop-cpp`.
//!
//! This file is *skeleton*: each module below is a compile placeholder that
//! will be filled with the port of the corresponding C++ translation unit.
//! Filled by GitHub Actions; see `.github/workflows/ci.yml`.

pub mod comparison {
    //! Port target: C++ source unit for `comparison`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::comparison"
    }
}

pub mod conflict_detector {
    //! Port target: C++ source unit for `conflict_detector`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::conflict_detector"
    }
}

pub mod diff_engine {
    //! Port target: C++ source unit for `diff_engine`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::diff_engine"
    }
}

pub mod diff_filter {
    //! Port target: C++ source unit for `diff_filter`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::diff_filter"
    }
}

pub mod equivalence_engine {
    //! Port target: C++ source unit for `equivalence_engine`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::equivalence_engine"
    }
}

pub mod match_engine {
    //! Port target: C++ source unit for `match_engine`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::match_engine"
    }
}

pub mod merge_engine {
    //! Port target: C++ source unit for `merge_engine`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::merge_engine"
    }
}

pub mod requirement_engine {
    //! Port target: C++ source unit for `requirement_engine`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-compare::requirement_engine"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(super::comparison::api_surface(), "emf-compare::comparison");
    }
}
