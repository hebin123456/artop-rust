//! `ComposedAdapterFactory` — composes several [`AdapterFactory`]s, delegating
//! in order (port of C++ `emf-edit` `provider/ComposedAdapterFactory`, aligned
//! to Java `org.eclipse.emf.edit.provider.ComposedAdapterFactory`).
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use emf_common::notification::{Adapter, AdapterFactory, NotifierHandle};

/// A factory that delegates to an ordered list of child factories (EMF
/// `ComposedAdapterFactory`).
#[derive(Default)]
pub struct ComposedAdapterFactory {
    factories: Vec<Box<dyn AdapterFactory>>,
}

impl ComposedAdapterFactory {
    /// An empty composed factory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a child factory (EMF `addAdapterFactory`).
    pub fn add_adapter_factory(&mut self, factory: Box<dyn AdapterFactory>) {
        self.factories.push(factory);
    }

    /// Remove a child factory by identity (EMF `removeAdapterFactory`).
    pub fn remove_adapter_factory(&mut self, factory: &dyn AdapterFactory) {
        let addr = factory as *const dyn AdapterFactory as *const ();
        self.factories
            .retain(|f| (f.as_ref() as *const dyn AdapterFactory as *const ()) != addr);
    }

    /// The child factories (EMF `getChildFactories`).
    pub fn child_factories(&self) -> &[Box<dyn AdapterFactory>] {
        &self.factories
    }
}

impl AdapterFactory for ComposedAdapterFactory {
    /// C++ `isFactoryForType` reports whether any child factory is present.
    fn is_factory_for_type(&self, _type_key: &str) -> bool {
        !self.factories.is_empty()
    }

    /// Delegate `adapt` to the first child factory that produces an adapter.
    fn adapt(
        &self,
        target: &NotifierHandle,
        existing: Option<&dyn Adapter>,
    ) -> Option<Box<dyn Adapter>> {
        self.factories
            .iter()
            .find_map(|f| f.adapt(target, existing))
    }

    /// Delegate `createAdapter` to the first child factory that produces one.
    fn create_adapter(&self, target: &NotifierHandle) -> Option<Box<dyn Adapter>> {
        self.factories.iter().find_map(|f| f.create_adapter(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_composed_factory_is_not_factory_for_type() {
        let caf = ComposedAdapterFactory::new();
        assert_eq!(caf.child_factories().len(), 0);
        assert!(!caf.is_factory_for_type(""));
    }

    #[test]
    fn adding_child_makes_it_factory_for_type() {
        struct NoopFactory;
        impl AdapterFactory for NoopFactory {}
        let mut caf = ComposedAdapterFactory::new();
        caf.add_adapter_factory(Box::new(NoopFactory));
        assert_eq!(caf.child_factories().len(), 1);
        assert!(caf.is_factory_for_type("anything"));
    }
}
