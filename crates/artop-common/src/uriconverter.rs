//! EMF `URIConverter` / `URIHandler`, ported from C++ `emf-common/URIConverter`
//! (aligned to Java `org.eclipse.emf.ecore.resource.URIConverter`).
//!
//! Normalizes URIs (applying a logical->physical map and resolving relative
//! URIs) and provides URI handling for serializers.

use crate::uri::Uri;
use std::collections::HashMap;

/// Converts / normalizes URIs.
#[derive(Debug, Clone, Default)]
pub struct UriConverter {
    uri_map: HashMap<String, String>,
}

impl UriConverter {
    /// New converter with an empty map.
    pub fn new() -> Self {
        Self {
            uri_map: HashMap::new(),
        }
    }

    /// Logical -> physical URI map.
    pub fn uri_map(&self) -> &HashMap<String, String> {
        &self.uri_map
    }

    /// Mutable logical -> physical URI map.
    pub fn uri_map_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.uri_map
    }

    /// Normalize a URI: apply the map and add `file:` prefix for relative paths.
    pub fn normalize(&self, uri: &Uri) -> Uri {
        let mut u = uri.clone();
        if let Some(mapped) = self.uri_map.get(&uri.to_string()) {
            u = Uri::parse(mapped);
        }
        if (u.is_relative() || u.path_is_relative()) && u.has_relative_path() {
            // A relative URI with . or .. gets an explicit file scheme.
            let path = u.path().to_string();
            u = Uri::create_file_uri(&path);
        }
        u
    }

    /// Resolve a relative URI against a base URI.
    pub fn resolve(&self, uri: &Uri, base: &Uri) -> Uri {
        uri.resolve(base)
    }

    /// Deresolve an absolute URI relative to a base.
    pub fn deresolve(&self, uri: &Uri, base: &Uri) -> Uri {
        uri.deresolve(base)
    }

    /// Whether a URI maps to an existing file.
    pub fn exists(&self, uri: &Uri) -> bool {
        let p = uri.to_file_path();
        p.is_empty() || std::path::Path::new(&p).exists()
    }
}

/// A URI handler bound to a base URI, used when serializing references.
#[derive(Debug, Clone, Default)]
pub struct UriHandler {
    base_uri: Option<Uri>,
}

impl UriHandler {
    /// New handler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base URI.
    pub fn set_base_uri(&mut self, uri: Uri) {
        self.base_uri = Some(uri);
    }

    /// The base URI.
    pub fn base_uri(&self) -> Option<&Uri> {
        self.base_uri.as_ref()
    }

    /// Resolve a relative URI against the base.
    pub fn resolve(&self, uri: &Uri) -> Uri {
        if let Some(base) = &self.base_uri {
            if !base.is_empty()
                && !base.is_relative()
                && uri.is_relative()
                && uri.has_relative_path()
            {
                return uri.resolve(base);
            }
        }
        uri.clone()
    }

    /// Deresolve an absolute URI against the base.
    pub fn deresolve(&self, uri: &Uri) -> Uri {
        if let Some(base) = &self.base_uri {
            if !base.is_empty() && !base.is_relative() && !uri.is_relative() {
                let d = uri.deresolve(base);
                if d.has_relative_path() {
                    return d;
                }
            }
        }
        uri.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_resolves_against_base() {
        let mut h = UriHandler::new();
        h.set_base_uri(Uri::parse("file:///a/b/c.xmi"));
        let r = h.resolve(&Uri::parse("../d.x"));
        // 对齐 artop-cpp 参考算法：`..` 不会被折叠回 base 段。
        assert_eq!(r.path(), "/a/b/d.x");
    }
}
