//! Headless core subset (port of C++ `emf-sphinx`).
//!
//! Port target: C++ `emf-sphinx` module of `hebin123456/artop-cpp`.
//!
//! [`headless_core`] provides the model-processing entry points that the
//! downstream patcher / validator / report tools are built on: a lightweight
//! object graph with path-based resolution and controllable depth-first
//! traversal.

pub mod headless_core;

pub use headless_core::{count, walk, Model, ModelError, Node, Root, WalkControl};
