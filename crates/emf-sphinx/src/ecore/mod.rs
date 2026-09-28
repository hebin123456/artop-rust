//! Ecore-level helpers.
//!
//! Port of C++ `emf-sphinx/ecore` (aligned to
//! `org.eclipse.sphinx.emf.ecore`). Only the pieces that are meaningful in the
//! headless Rust port are carried here.

pub mod ordered_feature_map;

pub use ordered_feature_map::{FeatureMapEntry, OrderedFeatureMap};
