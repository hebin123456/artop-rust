//! AUTOSAR release descriptor (port of C++ `AutosarReleaseDescriptor`).
//!
//! Aligned to Java `org.artop.aal.common.metamodel.AutosarReleaseDescriptor`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::version_data::AutosarMetaModelVersionData;

/// Metadata describing one AUTOSAR release: version, base namespace, schema file
/// naming, default content type and compatible releases.
#[derive(Debug, Clone)]
pub struct AutosarReleaseDescriptor {
    id: String,
    name: String,
    version_data: AutosarMetaModelVersionData,
    base_namespace: String,
    default_content_type_id: String,
    compatible_descriptors: Vec<AutosarReleaseDescriptor>,
    libraries: HashMap<String, String>,
}

impl AutosarReleaseDescriptor {
    /// Fallback descriptor id (Java/C++ `ID`).
    pub const ID: &'static str = "org.artop.aal.autosar.release";
    /// Base model name.
    pub const BASE_NAME: &'static str = "AUTOSAR";
    /// Base namespace URI for AUTOSAR R4.0.
    pub const BASE_NAMESPACE: &'static str = "http://autosar.org/schema/r4.0";
    /// Base content-type id.
    pub const ARXML_BASE_CONTENT_TYPE_ID: &'static str = "org.artop.aal.autosar.contenttype";
    /// Default arxml file extension.
    pub const ARXML_DEFAULT_FILE_EXTENSION: &'static str = "arxml";
    /// Schema file-name prefix.
    pub const AUTOSAR_SCHEMA_FILE_NAME_PREFIX: &'static str = "AUTOSAR_";
    /// Separator used in the new (4.4+) schema version segment.
    pub const AUTOSAR_SCHEMA_VERSION_NUMBER_SEPARATOR: &'static str = "-";
    /// XSD file extension.
    pub const XSD_FILE_EXTENSION: &'static str = "xsd";

    /// The global default ("fallback") descriptor (`INSTANCE`), version 4.4.0.
    pub fn instance() -> &'static AutosarReleaseDescriptor {
        static INSTANCE: OnceLock<AutosarReleaseDescriptor> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let mut d = AutosarReleaseDescriptor::new(
                Self::ID.to_string(),
                AutosarMetaModelVersionData::new(4, 4, 0),
            );
            d.set_name("AUTOSAR 4.4.0 (default)".to_string());
            d
        })
    }

    /// Construct with an id and version triple.
    pub fn new(id: String, version: AutosarMetaModelVersionData) -> Self {
        Self {
            id,
            name: String::new(),
            version_data: version,
            base_namespace: Self::BASE_NAMESPACE.to_string(),
            default_content_type_id: Self::ARXML_BASE_CONTENT_TYPE_ID.to_string(),
            compatible_descriptors: Vec::new(),
            libraries: HashMap::new(),
        }
    }

    /// Release id.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Display name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the display name.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Version triple.
    pub fn autosar_version_data(&self) -> &AutosarMetaModelVersionData {
        &self.version_data
    }
    /// Replace the version triple.
    pub fn set_autosar_version_data(&mut self, v: AutosarMetaModelVersionData) {
        self.version_data = v;
    }

    /// Base namespace URI.
    pub fn base_namespace(&self) -> &str {
        &self.base_namespace
    }
    /// Replace the base namespace URI.
    pub fn set_base_namespace(&mut self, ns: String) {
        self.base_namespace = ns;
    }

    /// `xsi:schemaLocation` base (`== base_namespace`).
    pub fn schema_location_base(&self) -> String {
        self.base_namespace.clone()
    }

    /// Full `xsi:schemaLocation` value, e.g.
    /// `"http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd"`.
    pub fn schema_location(&self) -> String {
        let version = if self.version_data.is_new_version() {
            self.version_data
                .schema_version_number_string(Self::AUTOSAR_SCHEMA_VERSION_NUMBER_SEPARATOR)
        } else {
            self.version_data.schema_version_number_string("")
        };
        format!(
            "{} {}{}.{}",
            self.schema_location_base(),
            Self::AUTOSAR_SCHEMA_FILE_NAME_PREFIX,
            version,
            Self::XSD_FILE_EXTENSION
        )
    }

    /// Whether `sl`'s first (space-separated) token equals the base namespace.
    pub fn matches_schema_location(&self, sl: &str) -> bool {
        if sl.is_empty() {
            return false;
        }
        let first = sl.split(' ').next().unwrap_or("");
        first == self.base_namespace
    }

    /// Concrete content-type id (`id + ".contenttype"` unless overridden).
    pub fn default_content_type_id(&self) -> String {
        if self.default_content_type_id == Self::ARXML_BASE_CONTENT_TYPE_ID && !self.id.is_empty() {
            return format!("{}.contenttype", self.id);
        }
        self.default_content_type_id.clone()
    }
    /// Override the content-type id.
    pub fn set_default_content_type_id(&mut self, id: String) {
        self.default_content_type_id = id;
    }

    /// Compatible release descriptors.
    pub fn compatible_descriptors(&self) -> &[AutosarReleaseDescriptor] {
        &self.compatible_descriptors
    }
    /// Append a compatible release descriptor.
    pub fn add_compatible_descriptor(&mut self, d: AutosarReleaseDescriptor) {
        self.compatible_descriptors.push(d);
    }

    /// Library descriptors (`id -> description`).
    pub fn library_descriptors(&self) -> &HashMap<String, String> {
        &self.libraries
    }
    /// Add a library descriptor.
    pub fn add_library_descriptor(&mut self, id: String, desc: String) {
        self.libraries.insert(id, desc);
    }

    /// Version-ascending comparison (`<0`, `0`, `>0`).
    pub fn compare_to(&self, other: &AutosarReleaseDescriptor) -> i32 {
        match self.version_data.cmp(&other.version_data) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        }
    }
}

impl PartialEq for AutosarReleaseDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.version_data == other.version_data
    }
}
impl Eq for AutosarReleaseDescriptor {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_location_old_and_new() {
        let d448 = AutosarReleaseDescriptor::new(
            "org.artop.aal.autosar448".to_string(),
            AutosarMetaModelVersionData::new(4, 4, 8),
        );
        assert_eq!(
            d448.schema_location(),
            "http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd"
        );
        let d40 = AutosarReleaseDescriptor::new(
            "org.artop.aal.autosar40".to_string(),
            AutosarMetaModelVersionData::new(4, 2, 1),
        );
        assert_eq!(
            d40.schema_location(),
            "http://autosar.org/schema/r4.0 AUTOSAR_00042.xsd"
        );
    }

    #[test]
    fn matches_schema_location_token() {
        let d = AutosarReleaseDescriptor::new(
            "x".to_string(),
            AutosarMetaModelVersionData::new(4, 4, 8),
        );
        assert!(d.matches_schema_location("http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd"));
        assert!(d.matches_schema_location("http://autosar.org/schema/r4.0"));
        assert!(!d.matches_schema_location("http://example.com/other"));
        assert!(!d.matches_schema_location(""));
    }

    #[test]
    fn fallback_instance() {
        assert_eq!(
            AutosarReleaseDescriptor::instance().id(),
            AutosarReleaseDescriptor::ID
        );
    }
}
