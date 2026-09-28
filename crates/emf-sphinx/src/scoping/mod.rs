//! Resource scoping: define the "model scope" a resource belongs to.
//!
//! Port of C++ `emf-sphinx/scoping` (aligned to
//! `org.eclipse.sphinx.emf.scoping`).

pub mod provider;
pub mod registry;
pub mod scope;

pub use provider::{FileResourceScopeProvider, ResourceScopeProvider};
pub use registry::ResourceScopeProviderRegistry;
pub use scope::{FileResourceScope, ResourceScope};
