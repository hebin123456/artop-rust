//! `ExtendedBasicExtendedMetaData` — cache keys for schema locations.
//!
//! Port of C++ `emf/sphinx/resource/ExtendedBasicExtendedMetaData.{h,cpp}`
//! (aligned to Java
//! `org.eclipse.sphinx.emf.resource.ExtendedBasicExtendedMetaData`).

/// Extended meta-data helper providing schema-location cache keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtendedBasicExtendedMetaData;

impl ExtendedBasicExtendedMetaData {
    /// The shared instance (aligned to C++ `instance()`).
    pub fn instance() -> Self {
        ExtendedBasicExtendedMetaData
    }

    /// Build a local cache key for a schema location: `ns|loc`, or just `loc`
    /// when the namespace is empty.
    pub fn get_cache_key(&self, ns: &str, loc: &str) -> String {
        if ns.is_empty() {
            loc.to_string()
        } else {
            format!("{ns}|{loc}")
        }
    }
}
