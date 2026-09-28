//! `ResourceScopeProviderRegistry` — registered scope providers.
//!
//! Port of C++ `emf/sphinx/scoping/ResourceScopeProviderRegistry.*` (aligned to
//! Java `org.eclipse.sphinx.emf.scoping.ResourceScopeProviderRegistry`).
//!
//! State lives in a thread-local so parallel tests stay isolated.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::resource::Resource;
use emf_common::uri::Uri;
use emf_common::value::ObjectRef;

use super::provider::ResourceScopeProvider;
use super::scope::ResourceScope;

thread_local! {
    static PROVIDERS: RefCell<Vec<Rc<dyn ResourceScopeProvider>>> = const { RefCell::new(Vec::new()) };
}

/// Registers all [`ResourceScopeProvider`]s and dispatches scope creation to
/// the first provider that yields a scope.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceScopeProviderRegistry;

impl ResourceScopeProviderRegistry {
    /// The singleton registry handle (aligned to C++ `instance()`).
    pub fn instance() -> Self {
        ResourceScopeProviderRegistry
    }

    /// Register a provider; duplicates (by handle identity) are ignored.
    pub fn register_provider(&self, provider: Rc<dyn ResourceScopeProvider>) {
        PROVIDERS.with(|p| {
            let mut p = p.borrow_mut();
            if !p.iter().any(|existing| Rc::ptr_eq(existing, &provider)) {
                p.push(provider);
            }
        });
    }

    /// Unregister a provider by handle identity.
    pub fn unregister_provider(&self, provider: &Rc<dyn ResourceScopeProvider>) {
        PROVIDERS.with(|p| {
            let mut p = p.borrow_mut();
            if let Some(i) = p.iter().position(|existing| Rc::ptr_eq(existing, provider)) {
                p.remove(i);
            }
        });
    }

    /// Create a scope for a resource via the first provider that succeeds.
    pub fn create_scope_from_resource(
        &self,
        res: Option<&Resource>,
    ) -> Option<Box<dyn ResourceScope>> {
        PROVIDERS.with(|p| {
            for provider in p.borrow().iter() {
                if let Some(scope) = provider.create_scope_from_resource(res) {
                    return Some(scope);
                }
            }
            None
        })
    }

    /// Create a scope for an object via the first provider that succeeds.
    pub fn create_scope_from_object(
        &self,
        obj: Option<&ObjectRef>,
    ) -> Option<Box<dyn ResourceScope>> {
        PROVIDERS.with(|p| {
            for provider in p.borrow().iter() {
                if let Some(scope) = provider.create_scope_from_object(obj) {
                    return Some(scope);
                }
            }
            None
        })
    }

    /// Create a scope for a URI via the first provider that succeeds.
    pub fn create_scope_from_uri(&self, uri: &Uri) -> Option<Box<dyn ResourceScope>> {
        PROVIDERS.with(|p| {
            for provider in p.borrow().iter() {
                if let Some(scope) = provider.create_scope_from_uri(uri) {
                    return Some(scope);
                }
            }
            None
        })
    }

    /// Whether `uri` falls outside every registered scope.
    pub fn is_not_in_any_scope(&self, uri: &Uri) -> bool {
        PROVIDERS.with(|p| {
            for provider in p.borrow().iter() {
                if let Some(scope) = provider.create_scope_from_uri(uri) {
                    if scope.belongs_to_uri(uri, false) {
                        return false;
                    }
                }
            }
            true
        })
    }
}
