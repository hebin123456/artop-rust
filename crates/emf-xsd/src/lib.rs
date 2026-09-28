//! XSD metamodel + parser — port of C++ `emf-xsd` (`hebin123456/artop-cpp`),
//! aligned to Java `org.eclipse.xsd`.
//!
//! The crate models an XML Schema document as a plain-data metamodel
//! ([`xsd_metamodel`]) and populates it from schema XML ([`xsd_parser`]). It is
//! a general-purpose, artop-agnostic building block: generated AUTOSAR
//! (`.arxml`) validation and `.ecore`/`.xsd` tooling sit on top of it.

pub mod pattern;
pub mod resource;
pub mod validator;
pub mod xsd_metamodel;
pub mod xsd_parser;

/// Resolve and incorporate XSD schema directives (`import`/`include`/`redefine`).
pub use resource::{SchemaLoader, XSDResource, XSDSchemaRegistry};
/// Validate an XML instance document against an [`XSDSchema`].
pub use validator::{XSDDiagnostic, XSDValidator, XSDValidatorOptions};
/// The `XSDSchema` root metamodel type.
pub use xsd_metamodel::XSDSchema;
/// Parse a schema document into an [`XSDSchema`].
pub use xsd_parser::parse_schema;
