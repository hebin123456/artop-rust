//! Acceleo MTL templates / M2T engine (port of C++ `emf-acceleo`).
//!
//! Port target: C++ `emf-acceleo` module of `hebin123456/artop-cpp`.
//!
//! This crate implements a lightweight Acceleo-style Model-to-Text (M2T)
//! engine:
//!
//! - [`mtl_parser`] parses `.mtl` template files into a list of template
//!   declarations with their parameters and body.
//! - [`template`] defines the template model (declaration, parameters, body,
//!   query) plus the runtime value context.
//! - [`m2t_engine`] evaluates a template against a value context and produces
//!   the generated text, supporting variable substitution, conditionals and
//!   iteration.

pub mod m2t_engine;
pub mod mtl_parser;
pub mod template;

pub use m2t_engine::{render, M2tError};
pub use mtl_parser::{parse_templates, TemplateParseError};
pub use template::{QueryDecl, TemplateDecl, TemplateFile, TemplateParam, ValueContext};
