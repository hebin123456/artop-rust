//! Acceleo MTL templates / M2T engine (port of C++ `emf-acceleo`).
//!
//! Port target: C++ `emf-acceleo` module of `hebin123456/artop-cpp`, aligned to
//! Java `org.eclipse.acceleo` and the AQL expression sub-set.
//!
//! This crate implements a lightweight Acceleo-style Model-to-Text (M2T)
//! engine:
//!
//! - [`ast`] defines the abstract syntax tree for template blocks
//!   (`text`/`expr`/`for`/`if`/`let`/`file`/`protected`) and the AQL expression
//!   sub-set (variables, literals, navigation, calls, lambdas).
//! - [`parser`] is the recursive-descent `.mtl` parser producing a [`Module`].
//! - [`engine`] evaluates templates against a value context and produces the
//!   generated text, including `[file]` output with `[protected]` region merge,
//!   registered services, module queries and the `extends` chain.
//!
//! `AcceleoEngine`/`AcceleoService` mirror the C++ pair of the same names; the
//! type-erased [`Val`](emf_common::value::Val) replaces C++ `std::any`, and
//! navigation (`c.name`) goes through the `EObject` reflection surface, so the
//! engine is metamodel agnostic.

pub mod ast;
pub mod engine;
pub mod parser;

pub use ast::{Block, Expr, FileBlock, ForBlock, IfBlock, LetBlock, Module, Param, ProtectedBlock};
pub use ast::{Query, Template};
pub use engine::{AcceleoEngine, AcceleoService, EngineError, EvalContext, ServiceFn};
pub use parser::{parse, AcceleoParseError};
