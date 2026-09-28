//! `SchemaLocationUriHandler` — parses `xsi:schemaLocation`.
//!
//! Port of C++ `emf/sphinx/resource/SchemaLocationURIHandler.{h,cpp}` (aligned
//! to Java `org.eclipse.sphinx.emf.resource.SchemaLocationURIHandler`).

use std::collections::BTreeMap;

use emf_xmi::XMIResource;

/// Parses and reads the `xsi:schemaLocation` attribute used for proxy
/// resolution.
#[derive(Debug, Clone, Copy, Default)]
pub struct SchemaLocationUriHandler;

impl SchemaLocationUriHandler {
    /// A new handler.
    pub fn new() -> Self {
        SchemaLocationUriHandler
    }

    /// Parse an `xsi:schemaLocation` string of `namespace systemId` pairs.
    ///
    /// An empty string or an odd number of whitespace-separated tokens yields
    /// an empty map (aligned to the C++ implementation).
    pub fn parse_schema_location(&self, schema_loc: &str) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();
        if schema_loc.is_empty() {
            return result;
        }
        let tokens: Vec<&str> = schema_loc.split_whitespace().collect();
        if !tokens.len().is_multiple_of(2) {
            return result;
        }
        for pair in tokens.chunks(2) {
            result.insert(pair[0].to_string(), pair[1].to_string());
        }
        result
    }

    /// Read the schema location stored on an XMI resource (`None` -> `""`).
    pub fn get_schema_location(&self, res: Option<&XMIResource>) -> String {
        match res {
            None => String::new(),
            Some(r) => r.get_xsi_schema_location().to_string(),
        }
    }
}
