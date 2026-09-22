//! # emf-common
//!
//! Foundational EMF infrastructure, ported from the C++ `emf-common` module of
//! `hebin123456/artop-cpp`. Mirrors the Eclipse EMF core concepts: `EObject`,
//! `EList`, `Resource`, `URI`, `Diagnostic`, `EPackageRegistry`, `EMap`,
//! `Command`, and the notification/feature-map primitives.
//!
//! This is the bottom of the dependency graph; every other crate builds on it.
//!
//! Design note: the C++ port builds on raw pointers + `std::any`. In Rust we
//! model object identity and references as `Rc<RefCell<dyn EObject>>`, and the
//! runtime value envelope is the [`value::Val`] enum (a type-safe `std::any`).
//! Inheritance stays *data* (see `autosar448-model`), following the established
//! porting strategy in this workspace.

pub mod command;
pub mod diagnostic;
pub mod elist;
pub mod emap;
pub mod eobject;
pub mod feature_map;
pub mod notification;
pub mod resource;
pub mod segment_sequence;
pub mod uri;
pub mod uriconverter;
pub mod value;
