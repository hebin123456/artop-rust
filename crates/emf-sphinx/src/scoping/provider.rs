//! `ResourceScopeProvider` interface + `FileResourceScopeProvider`.
//!
//! Port of C++ `emf/sphinx/scoping/IResourceScopeProvider.h` and
//! `FileResourceScopeProvider.*` (aligned to
//! `org.eclipse.sphinx.emf.scoping`).

use emf_common::resource::Resource;
use emf_common::uri::Uri;
use emf_common::value::ObjectRef;

use super::scope::{FileResourceScope, ResourceScope};

/// Creates a [`ResourceScope`] from a resource / object / URI.
pub trait ResourceScopeProvider {
    /// Create a scope for a resource (`None` -> `None`).
    fn create_scope_from_resource(&self, res: Option<&Resource>) -> Option<Box<dyn ResourceScope>>;

    /// Create a scope for an `EObject` (`None` -> `None`).
    ///
    /// C++ resolves `obj.eResource().getURI()`. The headless Rust object graph
    /// carries no resource back-link, so a non-null object currently yields
    /// `None` as well (documented parity gap).
    fn create_scope_from_object(&self, obj: Option<&ObjectRef>) -> Option<Box<dyn ResourceScope>>;

    /// Create a scope for a URI.
    fn create_scope_from_uri(&self, uri: &Uri) -> Option<Box<dyn ResourceScope>>;
}

/// Provides single-file scopes (aligned to `FileResourceScopeProvider`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FileResourceScopeProvider;

impl FileResourceScopeProvider {
    /// The shared instance (aligned to C++ `instance()`).
    pub fn instance() -> Self {
        FileResourceScopeProvider
    }
}

impl ResourceScopeProvider for FileResourceScopeProvider {
    fn create_scope_from_resource(&self, res: Option<&Resource>) -> Option<Box<dyn ResourceScope>> {
        let res = res?;
        Some(Box::new(FileResourceScope::new(res.uri().clone())))
    }

    fn create_scope_from_object(&self, obj: Option<&ObjectRef>) -> Option<Box<dyn ResourceScope>> {
        let _ = obj;
        None
    }

    fn create_scope_from_uri(&self, uri: &Uri) -> Option<Box<dyn ResourceScope>> {
        Some(Box::new(FileResourceScope::new(uri.clone())))
    }
}
