//! Meta-model-layer Sphinx extensions (port of C++ `emf-sphinx`).
//!
//! Port target: C++ `emf-sphinx` module of `hebin123456/artop-cpp`.
//!
//! This crate carries the headless, platform-independent parts of Sphinx:
//! - [`headless_core`]: a lightweight object graph with path resolution and
//!   controllable depth-first traversal (the entry point downstream patcher /
//!   validator / report tools build on);
//! - [`metamodel`]: meta-model descriptors, version data and the descriptor
//!   registry;
//! - [`resource`]: `xsi:schemaLocation` handling, extended meta-data and model
//!   converters;
//! - [`scoping`]: resource-scope definitions, providers and their registry;
//! - [`ecore`]: Ecore-level helpers (ordered feature maps);
//! - [`util`]: resource/URI helpers (`EcoreResourceUtil`).

pub mod ecore;
pub mod headless_core;
pub mod metamodel;
pub mod resource;
pub mod scoping;
pub mod util;

pub use headless_core::{count, walk, Model, ModelError, Node, Root, WalkControl};
