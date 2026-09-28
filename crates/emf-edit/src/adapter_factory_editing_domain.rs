//! `AdapterFactoryEditingDomain` — the default editing domain (port of C++
//! `emf-edit` `AdapterFactoryEditingDomain`, aligned to Java
//! `org.eclipse.emf.edit.domain.AdapterFactoryEditingDomain`).
//!
//! It carries the three collaborators an editing domain owns: an optional
//! [`AdapterFactory`] (to adapt model objects for display), an optional
//! command stack, and an optional resource set. All three default to `None`,
//! matching the C++ accessors that return `nullptr` on a freshly constructed
//! domain.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::command::BasicCommandStack;
use emf_common::notification::AdapterFactory;
use emf_common::resource::ResourceSet;

/// A model-editing domain that owns an adapter factory, a command stack and a
/// resource set (EMF `AdapterFactoryEditingDomain`).
#[derive(Default)]
pub struct AdapterFactoryEditingDomain {
    adapter_factory: RefCell<Option<Rc<dyn AdapterFactory>>>,
    command_stack: RefCell<Option<Rc<BasicCommandStack>>>,
    resource_set: RefCell<Option<Rc<RefCell<ResourceSet>>>>,
}

impl AdapterFactoryEditingDomain {
    /// A new domain with no factory, command stack or resource set
    /// (C++ default constructor: all accessors return `nullptr`).
    pub fn new() -> Self {
        Self::default()
    }

    /// The adapter factory, if any (EMF `getAdapterFactory`).
    pub fn get_adapter_factory(&self) -> Option<Rc<dyn AdapterFactory>> {
        self.adapter_factory.borrow().clone()
    }

    /// Set the adapter factory (EMF `setAdapterFactory`).
    pub fn set_adapter_factory(&self, factory: Option<Rc<dyn AdapterFactory>>) {
        *self.adapter_factory.borrow_mut() = factory;
    }

    /// The command stack, if any (EMF `getCommandStack`).
    pub fn get_command_stack(&self) -> Option<Rc<BasicCommandStack>> {
        self.command_stack.borrow().clone()
    }

    /// Set the command stack (EMF `setCommandStack`).
    pub fn set_command_stack(&self, stack: Option<Rc<BasicCommandStack>>) {
        *self.command_stack.borrow_mut() = stack;
    }

    /// The resource set, if any (EMF `getResourceSet`).
    pub fn get_resource_set(&self) -> Option<Rc<RefCell<ResourceSet>>> {
        self.resource_set.borrow().clone()
    }

    /// Set the resource set (EMF `setResourceSet`).
    pub fn set_resource_set(&self, set: Option<Rc<RefCell<ResourceSet>>>) {
        *self.resource_set.borrow_mut() = set;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_domain_has_no_collaborators() {
        let domain = AdapterFactoryEditingDomain::new();
        assert!(domain.get_adapter_factory().is_none());
        assert!(domain.get_command_stack().is_none());
        assert!(domain.get_resource_set().is_none());
    }

    #[test]
    fn set_command_stack_visible_via_getter() {
        let domain = AdapterFactoryEditingDomain::new();
        let stack = Rc::new(BasicCommandStack::new());
        domain.set_command_stack(Some(stack.clone()));
        assert!(domain.get_command_stack().is_some());
    }
}
