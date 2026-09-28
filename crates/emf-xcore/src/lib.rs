//! Xcore DSL parser (port of C++ `emf-xcore`).
//!
//! Port target: C++ `emf-xcore` module of `hebin123456/artop-cpp`.
//!
//! This crate implements the Xcore domain-specific language: a lightweight,
//! textual syntax for describing EMF metamodels. It owns four submodules:
//!
//! - [`dsl`]: the abstract syntax tree (AST) for an Xcore file, including
//!   packages, classes (entities), data types, enums, structural features and
//!   annotations.
//! - [`parser`]: the recursive-descent parser that turns Xcore source text into
//!   the AST, with references and validation.
//! - [`generator`]: derives Ecore metamodel instances (`EPackage`/`EClass`/...)
//!   from the AST, and emits GenModel XML.
//! - [`resource`]: `XcoreResource`/factory/standalone setup; loads `.xcore`
//!   text end-to-end into a derived `EPackage` + GenModel.

pub mod dsl;
pub mod generator;
pub mod parser;
pub mod resource;

pub use dsl::{
    Annotation, AnnotationDirective, AttributeDecl, ClassDecl, DataTypeDecl, EnumDecl,
    EnumLiteralDecl, OperationDecl, PackageDecl, ParameterDecl, ReferenceDecl, ReferenceKind,
};
pub use generator::{ecore_type_name, XcoreGenerator};
pub use parser::{parse, ParseError, ParsedFile};
pub use resource::{XcoreResource, XcoreResourceFactory, XcoreStandaloneSetup};
