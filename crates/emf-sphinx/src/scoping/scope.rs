//! `ResourceScope` interface + `FileResourceScope`.
//!
//! Port of C++ `emf/sphinx/scoping/IResourceScope.h` and `FileResourceScope.*`
//! (aligned to `org.eclipse.sphinx.emf.scoping`).

use emf_common::resource::Resource;
use emf_common::uri::Uri;

/// A model's resource scope: a root resource plus its referenced roots.
///
/// C++ overloads on `URI` vs `Resource*`; Rust makes the nullability explicit
/// with `Option<&Resource>`.
pub trait ResourceScope {
    /// The scope root URI.
    fn get_root_uri(&self) -> Uri;

    /// The referenced root URIs.
    fn get_referenced_root_uris(&self) -> Vec<Uri>;

    /// The referencing root URIs.
    fn get_referencing_root_uris(&self) -> Vec<Uri>;

    /// Whether `uri` belongs to this scope.
    fn belongs_to_uri(&self, uri: &Uri, include_referenced_scopes: bool) -> bool;

    /// Whether `res` belongs to this scope (`None` -> `false`).
    fn belongs_to_resource(&self, res: Option<&Resource>, include_referenced_scopes: bool) -> bool;

    /// The persisted files of this scope.
    fn get_persisted_files(&self, include_referenced_scopes: bool) -> Vec<Uri>;

    /// Historical `belongs_to` for a URI.
    fn did_belong_to_uri(&self, uri: &Uri, include_referenced_scopes: bool) -> bool;

    /// Historical `belongs_to` for a resource.
    fn did_belong_to_resource(
        &self,
        res: Option<&Resource>,
        include_referenced_scopes: bool,
    ) -> bool;

    /// Whether `uri` is shared with another scope.
    fn is_shared_uri(&self, uri: &Uri) -> bool;

    /// Whether `res` is shared with another scope.
    fn is_shared_resource(&self, res: Option<&Resource>) -> bool;
}

/// A single-file resource scope.
#[derive(Debug, Clone, PartialEq)]
pub struct FileResourceScope {
    file_uri: Uri,
}

impl FileResourceScope {
    /// A scope rooted at `file_uri`.
    pub fn new(file_uri: Uri) -> Self {
        Self { file_uri }
    }
}

impl Default for FileResourceScope {
    fn default() -> Self {
        Self {
            file_uri: Uri::new(),
        }
    }
}

impl ResourceScope for FileResourceScope {
    fn get_root_uri(&self) -> Uri {
        self.file_uri.clone()
    }

    fn get_referenced_root_uris(&self) -> Vec<Uri> {
        Vec::new()
    }

    fn get_referencing_root_uris(&self) -> Vec<Uri> {
        Vec::new()
    }

    fn belongs_to_uri(&self, uri: &Uri, _include_referenced_scopes: bool) -> bool {
        *uri == self.file_uri
    }

    fn belongs_to_resource(
        &self,
        res: Option<&Resource>,
        _include_referenced_scopes: bool,
    ) -> bool {
        match res {
            None => false,
            Some(r) => *r.uri() == self.file_uri,
        }
    }

    fn get_persisted_files(&self, _include_referenced_scopes: bool) -> Vec<Uri> {
        vec![self.file_uri.clone()]
    }

    fn did_belong_to_uri(&self, uri: &Uri, include_referenced_scopes: bool) -> bool {
        self.belongs_to_uri(uri, include_referenced_scopes)
    }

    fn did_belong_to_resource(
        &self,
        res: Option<&Resource>,
        include_referenced_scopes: bool,
    ) -> bool {
        self.belongs_to_resource(res, include_referenced_scopes)
    }

    fn is_shared_uri(&self, _uri: &Uri) -> bool {
        false
    }

    fn is_shared_resource(&self, _res: Option<&Resource>) -> bool {
        false
    }
}
