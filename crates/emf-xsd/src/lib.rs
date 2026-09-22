//! XSD metamodel + parser — port of C++ `emf-xsd` (`hebin123456/artop-cpp`),
//! aligned to Java `org.eclipse.xsd`.
//!
//! The crate models an XML Schema document as a plain-data metamodel
//! ([`xsd_metamodel`]) and populates it from schema XML ([`xsd_parser`]). It is
//! a general-purpose, artop-agnostic building block: generated AUTOSAR
//! (`.arxml`) validation and `.ecore`/`.xsd` tooling sit on top of it.

pub mod xsd_metamodel;
pub mod xsd_parser;

/// The `XSDSchema` root metamodel type.
pub use xsd_metamodel::XSDSchema;
/// Parse a schema document into an [`XSDSchema`].
pub use xsd_parser::parse_schema;
