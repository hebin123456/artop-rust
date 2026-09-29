//! AUTOSAR (ARTOP) runtime layer (port of C++ `emf-artop/emf-artop-runtime`).
//!
//! Aligned to Java `org.artop.aal.common.*`. Hosts the AUTOSAR release metadata
//! ([`release_descriptor`], [`version_data`]), the reflective helpers
//! ([`identifiable_util`], [`autosar_library_index`], [`unknown_element`]) and —
//! as the port progresses — the arxml serializer/deserializer and resource
//! machinery.
//!
//! This crate is the artop-specific layer: it is the only place that knows about
//! AUTOSAR. The `emf-*` crates below it stay generic.

pub mod autosar_library_index;
pub mod identifiable_util;
pub mod release_descriptor;
pub mod unknown_element;
pub mod version_data;

pub use autosar_library_index::AutosarLibraryIndex;
pub use identifiable_util::IdentifiableUtil;
pub use release_descriptor::AutosarReleaseDescriptor;
pub use unknown_element::UnknownElement;
pub use version_data::{AutosarMetaModelVersionData, VersionData};

#[cfg(test)]
mod tests {
    #[test]
    fn version_display() {
        let v = super::AutosarMetaModelVersionData::new(4, 4, 8);
        assert_eq!(v.to_string(), "4.4.8");
        assert_eq!(v.canonical_version_number(), (4 << 24) | (4 << 16) | 8);
    }
}
