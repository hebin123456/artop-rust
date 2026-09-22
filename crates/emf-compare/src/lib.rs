//! Model comparison (match+diff+merge) — port of C++ `emf-compare`
//! (`hebin123456/artop-cpp`), aligned to Java `org.eclipse.emf.compare`.
//!
//! The crate compares two (or three, via an "origin") `EObject` graphs and
//! reports the structural differences ([`Diff`], [`Comparison`]), so a model
//! tool can review, filter, merge or undo changes just like EMF Compare.
//!
//! Pipeline stages, each an engine module:
//! - [`match_engine`] — align objects across versions (ID / proximity);
//! - [`diff_engine`] — detect attribute / reference / move differences;
//! - [`equivalence_engine`] — group diffs that must merge together;
//! - [`conflict_detector`] — real vs pseudo conflicts in 3-way compares;
//! - [`requirement_engine`] — ordering constraints between diffs;
//! - [`merge_engine`] — apply diffs in dependency order;
//! - [`diff_filter`] — veto diffs before they reach the user.
//!
//! Everything is written over the `emf-common` reflection surface, so it is
//! independent of any domain metamodel (artop-free).

pub mod comparison;
pub mod conflict_detector;
pub mod diff;
pub mod diff_engine;
pub mod diff_filter;
pub mod equivalence_engine;
pub mod match_engine;
pub mod merge_engine;
pub mod requirement_engine;
pub mod support;

/// Convenience two-way compare (match + diff + equivalence).
pub use comparison::compare;
/// Convenience three-way compare (match + diff + conflict + equivalence).
pub use comparison::compare3;
/// The main result container.
pub use comparison::Comparison;
/// A single detected difference.
pub use diff::Diff;
