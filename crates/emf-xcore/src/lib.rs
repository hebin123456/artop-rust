//! Xcore DSL parser (port of C++ `emf-xcore`).
//!
//! Port target: C++ `emf-xcore` module of `hebin123456/artop-cpp`.
//!
//! This crate implements the Xcore domain-specific language: a lightweight,
//! textual syntax for describing EMF metamodels. It owns two submodules:
//!
//! - [`dsl`]: the abstract syntax tree (AST) for an Xcore file, including
//!   packages, classes (entities), data types, enums, structural features and
//!   annotations.
//! - [`parser`]: the recursive-descent parser that turns Xcore source text into
//!   the AST, with references and validation.

pub mod dsl;
pub mod parser;

pub use dsl::{
    Annotation, DataTypeDecl, EClassDecl, EEnumDecl, EEnumLiteralDecl, FeatureDecl, PackageDecl,
    TypedElement,
};
pub use parser::{parse, ParseError, ParsedFile};
