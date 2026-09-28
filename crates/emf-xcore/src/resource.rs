//! `XcoreResource` / `XcoreResourceFactory` / `XcoreStandaloneSetup`.
//!
//! Port of C++ `emf-xcore/src/XcoreResource.cpp` (aligned to Java
//! `org.eclipse.emf.ecore.xcore.resource.XcoreResource`).
//!
//! Loading an `.xcore` file:
//!
//! 1. read the source text;
//! 2. [`crate::parser::parse`] it into a [`PackageDecl`] AST;
//! 3. [`crate::generator::XcoreGenerator`] derive the Ecore `EPackage`;
//! 4. derive the GenModel XML;
//! 5. expose the derived `EPackage` as the resource's single content.
//!
//! Design note: the C++ `XcoreResource` extends the raw-pointer `Resource` base
//! and stores `EPackage*` (an `EObject` subtype in C++). In this port `EPackage`
//! is a value type (`PackageRef`), not an `EObject`, so `XcoreResource` is a
//! standalone struct that owns its derived package rather than pushing it into
//! `emf-common`'s `ObjectRef`-based `Resource.contents`. [`XcoreResource::contents`]
//! still reports the derived package as the single content, mirroring
//! `contents().size() == 1`.
//!
//! The `.xcore` suffix → factory registry is a process-global table, matching
//! the C++ `XcoreResourceFactory` static registry.

use crate::dsl::PackageDecl;
use crate::generator::XcoreGenerator;
use crate::parser::{parse, ParseError};
use emf_common::uri::Uri;
use emf_ecore::PackageRef;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A resource that loads `.xcore` text and derives an Ecore `EPackage` plus a
/// GenModel XML document (C++ `XcoreResource`).
#[derive(Debug, Clone, Default)]
pub struct XcoreResource {
    uri: Uri,
    source: String,
    xpackage: Option<PackageDecl>,
    package: Option<PackageRef>,
    gen_model: String,
    loaded: bool,
}

impl XcoreResource {
    /// New, empty resource at `uri`.
    pub fn new(uri: Uri) -> Self {
        Self {
            uri,
            ..Self::default()
        }
    }

    /// The resource URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Set the resource URI.
    pub fn set_uri(&mut self, uri: Uri) {
        self.uri = uri;
    }

    /// Whether a source text has been successfully loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// The parsed Xcore AST, if loaded (C++ `xpackage_`).
    pub fn xpackage(&self) -> Option<&PackageDecl> {
        self.xpackage.as_ref()
    }

    /// The derived Ecore package, if loaded (C++ `derivedPackage_`).
    pub fn package(&self) -> Option<&PackageRef> {
        self.package.as_ref()
    }

    /// The derived Ecore package, if loaded (alias for [`Self::package`]).
    pub fn epackage(&self) -> Option<&PackageRef> {
        self.package.as_ref()
    }

    /// The derived GenModel XML text (C++ `genModel_`).
    pub fn gen_model(&self) -> &str {
        &self.gen_model
    }

    /// The resource contents. `EPackage` is not an `EObject` in this port, so
    /// the derived package is returned as a one-element vector (or empty before
    /// a successful load), mirroring C++ `contents().size()`.
    pub fn contents(&self) -> Vec<PackageRef> {
        self.package.iter().cloned().collect()
    }

    /// Load from in-memory Xcore source text (C++ `load(std::istream&)`),
    /// parsing, deriving the `EPackage` and generating the GenModel XML.
    pub fn load_str(&mut self, source: &str) -> Result<(), ParseError> {
        self.source = source.to_string();
        let parsed = parse(source)?;
        let xpackage = parsed.package.unwrap_or_default();

        let generator = XcoreGenerator::new();
        let package = generator.generate(&xpackage);
        let gen_model = generator.generate_gen_model(&xpackage);

        self.xpackage = Some(xpackage);
        self.package = Some(package);
        self.gen_model = gen_model;
        self.loaded = true;
        Ok(())
    }

    /// Load from a file path on disk.
    pub fn load_file(&mut self, path: &str) -> Result<(), String> {
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.load_str(&source).map_err(|e| e.to_string())
    }
}

/// The process-global `.xcore` factory registry (C++ `XcoreResourceFactory`).
type Factory = fn(&Uri) -> XcoreResource;

fn registry() -> &'static Mutex<HashMap<String, Factory>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Factory>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Factory for `.xcore` resources; registers the `xcore` file extension.
pub struct XcoreResourceFactory;

impl XcoreResourceFactory {
    /// Register the `xcore` extension factory (idempotent).
    pub fn register_factory() {
        let mut r = registry().lock().expect("xcore factory registry");
        r.entry("xcore".to_string())
            .or_insert(|uri| XcoreResource::new(uri.clone()));
    }

    /// Create a resource for `uri` if its extension is registered; `None`
    /// otherwise (C++ `XcoreResourceFactory::createResourceFor`).
    pub fn create_resource_for(uri: &Uri) -> Option<XcoreResource> {
        let s = uri.to_string();
        let dot = s.rfind('.')?;
        if dot + 1 >= s.len() {
            return None;
        }
        let ext = &s[dot + 1..];
        let r = registry().lock().expect("xcore factory registry");
        r.get(ext).map(|f| f(uri))
    }
}

/// Standalone bootstrap: initialize Ecore and register the `.xcore` factory
/// (C++ `XcoreStandaloneSetup::setup`). Idempotent.
pub struct XcoreStandaloneSetup;

impl XcoreStandaloneSetup {
    /// Run the standalone setup.
    pub fn setup() {
        // Initialize the Ecore meta-package that Xcore derives onto.
        let _ = emf_ecore::ecore_package::ecore_package();
        XcoreResourceFactory::register_factory();
    }
}
