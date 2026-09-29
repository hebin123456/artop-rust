//! `UnknownElement` — a lightweight record of an unmapped XML element.
//!
//! Port of C++ `emf::artop::runtime::UnknownElement` (aligned to Java
//! `org.eclipse.emf.ecore.xmi.UnknownFeature` / `FEATURE_MAP_UNKNOWN`).
//!
//! When `AutosarXMLLoader` encounters an XML element that cannot be mapped to an
//! `EStructuralFeature` (with `OPTION_RECORD_UNKNOWN_FEATURE = true`), it records
//! it here: tag name, attribute key/value pairs, text content and recursive child
//! records. The records are kept for later diagnostics / round-trip preservation.

use std::collections::HashMap;

/// A lightweight record of an unknown XML element (plain data, not an `EObject`).
#[derive(Debug, Clone, Default)]
pub struct UnknownElement {
    /// The XML element name (including any prefix).
    pub tag_name: String,
    /// Attribute key/value pairs.
    pub attributes: HashMap<String, String>,
    /// Text content.
    pub text: String,
    /// Child elements.
    pub children: Vec<UnknownElement>,
}

impl UnknownElement {
    /// Create a new record for `tag`.
    pub fn create(tag: impl Into<String>) -> Self {
        Self {
            tag_name: tag.into(),
            ..Self::default()
        }
    }

    /// Append a child element, returning a mutable handle to it.
    pub fn add_child(&mut self, child: UnknownElement) -> &mut UnknownElement {
        self.children.push(child);
        let last = self.children.len() - 1;
        &mut self.children[last]
    }

    /// Set an attribute.
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// Look up an attribute value.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_a_nested_unknown_element() {
        let mut root = UnknownElement::create("SOME-UNKNOWN");
        root.set_attribute("DEST", "Foo");
        root.text = "hello".to_string();
        root.add_child(UnknownElement::create("CHILD"));
        assert_eq!(root.tag_name, "SOME-UNKNOWN");
        assert_eq!(root.get_attribute("DEST"), Some("Foo"));
        assert_eq!(root.get_attribute("NOPE"), None);
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].tag_name, "CHILD");
        assert_eq!(root.text, "hello");
    }
}
