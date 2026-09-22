//! Port of C++ `emf-ecore-codegen`'s model-loading + code-generation layer.
//!
//! The convenient entry point is [`GenModel`]: load an `.ecore` document
//! (string or file) and turn it into a self-contained, compilable Rust crate:
//!
//! ```
//! use emf_ecore_codegen::{CrateSpec, GenModel};
//! let model = GenModel::load(include_str!("../samples/library.ecore"))?;
//! # let dir = std::env::temp_dir().join("codegen_doc_example");
//! model.generate_crate(&dir, &CrateSpec::default())?;
//! # let _ = std::fs::remove_dir_all(&dir);
//! # Ok::<(), String>(())
//! ```
//!
//! The pipeline (mirrors `emf-ecore-codegen`'s `GenModel` / `CppGenerator`)
//! is exposed one layer lower for programmatic use:
//!
//! 1. [`loader::load_ecore_package`] parses an `.ecore` document — which is
//!    itself an XMI instance of the Ecore metamodel — into an owned
//!    [`emf_ecore::EPackage`] (registering classes, data types and enums).
//! 2. [`generator::generate`] walks that `EPackage` and emits idiomatic Rust
//!    source: one statically-typed struct per `EClass`, a `match`-based EObject
//!    reflection table per class (compile-time dispatch, no dynamic string
//!    lookup), plus a `Package`/`EFactory` block — so the generated code
//!    compiles independently of the original `.ecore` file.
//!
//! # Compile-time strategy for large models
//!
//! Unlike C++ (one `.cpp` per class, `-O0` per unit) and Java (one `.java` per
//! class), Rust compiles a *crate* as its unit. To keep type-checking time
//! acceptable when a model grows large, this port:
//! - keeps each generated class a plain `struct` with concrete field types
//!   (no macros, no blanket generic impls, no derive-stacking);
//! - uses a `match feature_name` reflection table per class — a flat compile
//!   time dispatch equivalent to C++'s `switch (featureID)` — never a linear
//!   string scan at runtime;
//! - splits generated output per package into an independent thin crate, so
//!   the workspace can compile many packages in parallel and incrementally.

pub mod gen_model;
pub mod generator;
pub mod loader;
pub mod typing;

pub use gen_model::{CrateSpec, GenModel};
