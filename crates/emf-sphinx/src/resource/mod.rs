//! Resource-layer extensions: schema-location handling, extended meta-data and
//! model converters.
//!
//! Port of C++ `emf-sphinx/resource` (aligned to
//! `org.eclipse.sphinx.emf.resource`).

pub mod extended_meta_data;
pub mod model_converter;
pub mod schema_location;

pub use extended_meta_data::ExtendedBasicExtendedMetaData;
pub use model_converter::{ModelConverter, ModelConverterRegistry};
pub use schema_location::SchemaLocationUriHandler;
