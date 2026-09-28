//! `EcoreResourceUtil` — resource load/save and URI helpers.
//!
//! Port of C++ `emf/sphinx/util/EcoreResourceUtil.{h,cpp}` (aligned to Java
//! `org.eclipse.sphinx.emf.util.EcoreResourceUtil`).
//!
//! The C++ implementation is an explicit headless skeleton: several methods
//! return empty results because the Eclipse-platform machinery they delegate to
//! has no headless counterpart. Those branches are reproduced here and called
//! out per-method; the parity tests in `tests/cpp_parity_sphinx.rs` pin them.

use std::collections::BTreeMap;

use emf_common::resource::{Resource, ResourceHandle, ResourceSet};
use emf_common::uri::Uri;
use emf_common::uriconverter::UriConverter;
use emf_common::value::{ObjectRef, Val};

/// Load/save options (C++ `std::map<std::string, std::any>`).
pub type ResourceOptions = BTreeMap<String, Val>;

/// Static resource/URI helpers.
#[derive(Debug, Clone, Copy, Default)]
pub struct EcoreResourceUtil;

impl EcoreResourceUtil {
    /// The URI converter (C++ `getURIConverter`). In headless mode there is no
    /// workspace → `platform:/resource` mapping, so this is a plain converter.
    pub fn get_uri_converter(_rs: Option<&ResourceSet>) -> UriConverter {
        UriConverter::new()
    }

    /// Convert a relative URI to an absolute `file:` URI (C++
    /// `convertToAbsoluteFileURI`). Absolute URIs are returned unchanged.
    pub fn convert_to_absolute_file_uri(uri: &Uri) -> Uri {
        if uri.is_relative() {
            Self::get_uri_converter(None).normalize(uri)
        } else {
            uri.clone()
        }
    }

    /// Convert to a `platform:/resource` URI (C++
    /// `convertToPlatformResourceURI`). Headless: just normalize.
    pub fn convert_to_platform_resource_uri(uri: &Uri) -> Uri {
        Self::get_uri_converter(None).normalize(uri)
    }

    /// Whether the URI maps to an existing file (C++ `exists`).
    pub fn exists(uri: &Uri) -> bool {
        Self::get_uri_converter(None).exists(uri)
    }

    /// The URI of an object (C++ `getURI`). The headless object graph carries
    /// no resource back-link, so this returns an empty URI in every case,
    /// matching the C++ skeleton's `TODO`.
    pub fn get_uri(obj: Option<&ObjectRef>) -> Uri {
        let _ = obj;
        Uri::new()
    }

    /// Normalize a URI fragment against a resource (C++
    /// `normalizeURIFragment`). The C++ skeleton returns the fragment unchanged
    /// (its `ExtendedResourceAdapter` path is a `TODO`); so does this port.
    pub fn normalize_uri_fragment(res: Option<&Resource>, fragment: &str) -> String {
        let _ = res;
        fragment.to_string()
    }

    /// Read the model namespace of a resource (C++ `readModelNamespace`): the
    /// value of the first `xmlns="..."` declaration found in the file.
    pub fn read_model_namespace(res: Option<&Resource>) -> String {
        match res {
            None => String::new(),
            Some(r) => Self::read_namespace_from_uri(r.uri()),
        }
    }

    /// Read the target namespace of a resource (C++ `readTargetNamespace`).
    /// The C++ skeleton delegates to `readModelNamespace`.
    pub fn read_target_namespace(res: Option<&Resource>) -> String {
        Self::read_model_namespace(res)
    }

    /// Read the root-element comments (C++ `readRootElementComments`). The C++
    /// skeleton returns an empty list.
    pub fn read_root_element_comments(res: Option<&Resource>) -> Vec<String> {
        let _ = res;
        Vec::new()
    }

    /// Read the `xsi:schemaLocation` entries (C++
    /// `readSchemaLocationEntries`). The C++ skeleton returns an empty map.
    pub fn read_schema_location_entries(res: Option<&Resource>) -> BTreeMap<String, String> {
        let _ = res;
        BTreeMap::new()
    }

    /// The default load options (C++ `getDefaultLoadOptions`): record unknown
    /// features instead of failing.
    pub fn get_default_load_options() -> ResourceOptions {
        let mut options = ResourceOptions::new();
        options.insert("RECORD_UNKNOWN_FEATURE".to_string(), Val::Bool(true));
        options
    }

    /// The default save options (C++ `getDefaultSaveOptions`): empty.
    pub fn get_default_save_options() -> ResourceOptions {
        ResourceOptions::new()
    }

    /// Load (or fetch) a resource from a set (C++ `loadResource`). A `None` set
    /// yields `Ok(None)`.
    pub fn load_resource<'a>(
        rs: Option<&'a mut ResourceSet>,
        uri: &Uri,
        _options: &ResourceOptions,
    ) -> Result<Option<&'a dyn ResourceHandle>, String> {
        let Some(rs) = rs else {
            return Ok(None);
        };
        let normalized = Self::convert_to_platform_resource_uri(uri);
        Ok(rs.get_resource(&normalized, true)?.map(|h| h.as_ref()))
    }

    /// Load an `EObject` by URI (C++ `loadEObject`): delegates to
    /// [`Self::get_eobject`].
    pub fn load_eobject(rs: Option<&ResourceSet>, uri: &Uri) -> Option<ObjectRef> {
        Self::get_eobject(rs, uri)
    }

    /// Resolve an `EObject` from a set by URI (C++ `getEObject`). A `None` set
    /// or an empty fragment yields `None`.
    pub fn get_eobject(rs: Option<&ResourceSet>, uri: &Uri) -> Option<ObjectRef> {
        let rs = rs?;
        if uri.fragment().is_empty() {
            return None;
        }
        let base = uri.trim_fragment();
        let handle = rs.resources().iter().find(|r| *r.uri() == base)?;
        // Only plain in-memory resources expose fragment resolution through the
        // shared handle surface; serializer-backed handles answer `None`.
        let res = handle.as_any().downcast_ref::<Resource>()?;
        res.get_eobject(uri.fragment())
    }

    /// The first root object of a resource (C++ `getModelRoot`).
    pub fn get_model_root(res: Option<&Resource>) -> Option<ObjectRef> {
        let res = res?;
        res.contents().first().cloned()
    }

    /// Whether a resource held by the set is loaded (C++
    /// `isResourceLoaded`).
    pub fn is_resource_loaded(rs: Option<&ResourceSet>, uri: &Uri) -> bool {
        let Some(rs) = rs else {
            return false;
        };
        rs.resources()
            .iter()
            .find(|r| r.uri() == uri)
            .is_some_and(|r| r.is_loaded())
    }

    /// The model name derived from an object's package (C++ `getModelName`).
    /// The headless object graph has no `EPackage` back-link, so this returns
    /// `""`, matching the C++ skeleton's null-package branch.
    pub fn get_model_name(obj: Option<&ObjectRef>) -> String {
        let _ = obj;
        String::new()
    }

    /// Create a resource for `uri`, add it to the set and set `content` as its
    /// root (C++ `addNewModelResource`). A `None` set or `None` content yields
    /// `None`.
    pub fn add_new_model_resource<'a>(
        rs: Option<&'a mut ResourceSet>,
        uri: &Uri,
        _content_type_id: &str,
        content: Option<ObjectRef>,
    ) -> Option<&'a mut Box<dyn ResourceHandle>> {
        let rs = rs?;
        let content = content?;
        let resource = rs.create_resource(uri.clone());
        resource.set_contents(vec![content]);
        Some(resource)
    }

    /// Add an already-built resource to a set (C++ `addModelResource`). The
    /// Rust `ResourceSet` owns its handles, so a resource built outside the set
    /// cannot be adopted; this is a checked no-op.
    pub fn add_model_resource(rs: Option<&ResourceSet>, res: Option<&Resource>) {
        let _ = (rs, res);
    }

    /// Create a resource, add it to the set, then save it (C++
    /// `saveNewModelResource`).
    pub fn save_new_model_resource(
        rs: Option<&mut ResourceSet>,
        uri: &Uri,
        content_type_id: &str,
        content: Option<ObjectRef>,
        _options: &ResourceOptions,
    ) -> Result<(), String> {
        if let Some(handle) = Self::add_new_model_resource(rs, uri, content_type_id, content) {
            handle.save()?;
        }
        Ok(())
    }

    /// Save a resource (C++ `saveModelResource`). A `None` resource is a no-op;
    /// a save failure is surfaced as `Err` (the C++ version rethrows).
    pub fn save_model_resource(
        res: Option<&mut dyn ResourceHandle>,
        _options: &ResourceOptions,
    ) -> Result<(), String> {
        match res {
            None => Ok(()),
            Some(r) => r.save(),
        }
    }

    /// Unload a resource (C++ `unloadResource(Resource*)`). The headless Rust
    /// `Resource` has no unload, so this is a no-op.
    pub fn unload_resource(res: Option<&mut Resource>, _memory_optimized: bool) {
        let _ = res;
    }

    /// Unload the resource at `uri` from a set (C++
    /// `unloadResource(ResourceSet*, URI)`). No-op, as above.
    pub fn unload_resource_from_set(rs: Option<&ResourceSet>, uri: &Uri, memory_optimized: bool) {
        let _ = (rs, uri, memory_optimized);
    }

    /// Scan a file for the first `xmlns="..."` declaration (the C++
    /// `readModelNamespace(URIConverter*, URI)` body).
    fn read_namespace_from_uri(uri: &Uri) -> String {
        if !Self::exists(uri) {
            return String::new();
        }
        let Ok(text) = std::fs::read_to_string(uri.to_file_path()) else {
            return String::new();
        };
        for line in text.lines() {
            if !line.contains("xmlns=") {
                continue;
            }
            let Some(start) = line.find("xmlns=\"") else {
                continue;
            };
            let rest = &line[start + "xmlns=\"".len()..];
            let Some(end) = rest.find('"') else {
                return String::new();
            };
            return rest[..end].to_string();
        }
        String::new()
    }
}
