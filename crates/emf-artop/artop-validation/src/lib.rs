//! # artop-validation
//!
//! AUTOSAR *business* constraints, layered *above* the generic
//! `emf-validation` base (aligned to the real artop layering
//! `org.artop.aal.*.constraints`, where AUTOSAR-domain constraint bundles live
//! as plugins on top of the EMF validation runtime rather than polluting it).
//!
//! The generic `emf-validation` crate provides the *mechanism* (constraint
//! registration, mode dispatch, severeity, evaluators) with no AUTOSAR
//! vocabulary. This crate supplies the AUTOSAR *vocabulary* — the reflective
//! constraints that inspect `shortName` / `uuid` / `category` and cross
//! references through the `emf_common::eobject::EObject` surface, so they work
//! on real AUTOSAR models and on the dynamic test model alike.
//!
//! Layering / dependency rule: this crate depends on `emf-common`
//! (reflection + diagnostics) and `emf-validation` (constraint runtime) *only*.
//! It deliberately does **not** depend on `emf-ecore` in its public API — the
//! constraints are written against the `EObject` trait so they stay
//! model-technical agnostic (the same principle as the C++ port and the real
//! `org.artop.aal.*.constraints` plugins).

pub mod autosar_constraints;

pub use autosar_constraints::{
    register_autosar_constraints, register_ecuc_constraints, validate_named_single,
    validate_named_tree, validate_uuid_uniqueness,
};