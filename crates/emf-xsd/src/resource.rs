//! XSD resource + schema registry — port of C++ `emf-xsd`
//! (`XSDResource.cpp` / `XSDSchemaRegistry`), aligned to Java
//! `org.eclipse.xsd`'s schema resolution and incorporation.
//!
//! A [`XSDSchemaRegistry`] maps `schemaLocation` strings to loaded
//! [`XSDSchemaRef`]s; when a location is not cached it delegates to an optional
//! user-supplied loader. [`XSDResource::resolve_and_incorporate`] drives the
//! import/include/redefine protocol: it resolves the directive's
//! `schemaLocation` and hands the schema to [`XSDSchema::incorporate`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::xsd_metamodel::{XSDSchema, XSDSchemaCompositor, XSDSchemaRef, XsdDirectiveKind};

/// Callback used by [`XSDSchemaRegistry`] to load a schema on demand
/// (C++ `XSDSchemaRegistry::Loader`).
pub type SchemaLoader = Box<dyn Fn(&str) -> Option<XSDSchemaRef>>;

thread_local! {
    /// Process/thread-global registry backing [`XSDSchemaRegistry::with_global`]
    /// (C++ `XSDSchemaRegistry::instance()`'s singleton).
    static GLOBAL_REGISTRY: RefCell<XSDSchemaRegistry> =
        RefCell::new(XSDSchemaRegistry::new());
}

/// A `schemaLocation` → schema cache with an optional on-demand loader
/// (C++ `emf::xsd::XSDSchemaRegistry`).
#[derive(Default)]
pub struct XSDSchemaRegistry {
    schemas: HashMap<String, XSDSchemaRef>,
    loader: Option<SchemaLoader>,
}

impl XSDSchemaRegistry {
    /// New empty registry with no loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `schema` under `schemaLocation`. Empty locations are ignored
    /// (C++ `registerSchema`).
    pub fn register_schema(&mut self, schema_location: &str, schema: XSDSchemaRef) {
        if schema_location.is_empty() {
            return;
        }
        self.schemas.insert(schema_location.to_string(), schema);
    }

    /// The schema registered under `schema_location`, if any
    /// (C++ `findByLocation`).
    pub fn find_by_location(&self, schema_location: &str) -> Option<XSDSchemaRef> {
        self.schemas.get(schema_location).cloned()
    }

    /// Drop every cached schema and the loader (C++ `clear`).
    pub fn clear(&mut self) {
        self.schemas.clear();
        self.loader = None;
    }

    /// Install the on-demand loader (C++ `setLoader`).
    pub fn set_loader(&mut self, loader: SchemaLoader) {
        self.loader = Some(loader);
    }

    /// Ask the loader for `schema_location`; `None` without a loader or when the
    /// loader declines (C++ `load`).
    pub fn load(&self, schema_location: &str) -> Option<XSDSchemaRef> {
        self.loader
            .as_ref()
            .and_then(|loader| loader(schema_location))
    }

    /// Run `f` against the process/thread-global registry
    /// (C++ `XSDSchemaRegistry::instance()`).
    pub fn with_global<R>(f: impl FnOnce(&mut XSDSchemaRegistry) -> R) -> R {
        GLOBAL_REGISTRY.with(|global| f(&mut global.borrow_mut()))
    }
}

/// Resolves and incorporates XSD schema directives
/// (C++ `emf::xsd::XSDResource`).
pub struct XSDResource;

impl XSDResource {
    /// Resolve `schema_location` via the global registry: cached schemas win,
    /// otherwise the loader is consulted (C++ `XSDResource::resolveSchema`).
    pub fn resolve_schema(schema_location: &str) -> Option<XSDSchemaRef> {
        if schema_location.is_empty() {
            return None;
        }
        XSDSchemaRegistry::with_global(|registry| {
            if let Some(cached) = registry.find_by_location(schema_location) {
                return Some(cached);
            }
            registry.load(schema_location)
        })
    }

    /// Resolve the schema named by `compositor` and incorporate it
    /// (C++ `XSDResource::resolveAndIncorporate`).
    ///
    /// The caller is expected to have set `compositor.schema` to the owning
    /// (referencing) schema. Returns the incorporated schema, or `None` when the
    /// directive carries no usable `schemaLocation` or nothing could be resolved.
    ///
    /// An `xs:import` that names only a namespace (no `schemaLocation`) is
    /// treated as already resolved: its `incorporated_schema` is cleared and
    /// `None` is returned, matching the C++ behaviour.
    pub fn resolve_and_incorporate(compositor: &mut XSDSchemaCompositor) -> Option<XSDSchemaRef> {
        let location = match compositor.kind {
            XsdDirectiveKind::Import => match compositor.schema_location.as_deref() {
                Some(loc) if !loc.is_empty() => loc.to_string(),
                // Namespace-only import — nothing to load.
                _ => {
                    compositor.incorporated_schema = None;
                    return None;
                }
            },
            XsdDirectiveKind::Include | XsdDirectiveKind::Redefine => {
                match compositor.schema_location.as_deref() {
                    Some(loc) if !loc.is_empty() => loc.to_string(),
                    _ => return None,
                }
            }
        };

        let schema = Self::resolve_schema(&location)?;
        compositor.resolved_schema = Some(Rc::clone(&schema));
        XSDSchema::incorporate(&schema, compositor);
        Some(schema)
    }

    /// Parse an in-memory schema document into a shared schema handle
    /// (C++ `XSDResource::parseSchemaFromString`).
    pub fn parse_schema_from_string(xml: &str) -> Result<XSDSchemaRef, String> {
        crate::xsd_parser::parse_schema(xml).map(|schema| Rc::new(RefCell::new(schema)))
    }
}
