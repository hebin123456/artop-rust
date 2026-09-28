//! `TransactionalEditingDomain` — an editing domain with read/write
//! transactions and deferred change notification (port of C++
//! `emf-edit` `TransactionalEditingDomain`, aligned to Java
//! `org.eclipse.emf.transaction.TransactionalEditingDomain`).
//!
//! A write transaction (`run_write`) defers notification delivery: while the
//! outermost transaction is open, notifiers accumulate their notifications in
//! the `emf-common` deferral queue instead of delivering them. When the
//! outermost transaction commits, the accumulated notifications are coalesced
//! (same-notifier/feature `SET`s merge, `ADD`/`REMOVE` cancel) and delivered in
//! one batch. Nested transactions are re-entrant and only the outermost one
//! commits, matching the C++ thread-local transaction-depth behaviour.
//!
//! A read transaction (`run_exclusive`) takes no model-mutating action; it
//! exists so read-only code paths can share the same domain API.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use std::cell::Cell;

use emf_common::notification::{
    flush_deferred_notifications, is_delivery_deferred, set_delivery_deferred,
};

use super::adapter_factory_editing_domain::AdapterFactoryEditingDomain;

/// A transactional editing domain (EMF `TransactionalEditingDomain`).
#[derive(Default)]
pub struct TransactionalEditingDomain {
    base: AdapterFactoryEditingDomain,
    /// Nested-transaction depth; the outermost transaction commits (C++
    /// `tlsTransactionDepth`).
    depth: Cell<i32>,
}

impl TransactionalEditingDomain {
    /// A new domain with no open transaction.
    pub fn new() -> Self {
        Self::default()
    }

    /// The underlying [`AdapterFactoryEditingDomain`] (workpace/command-stack
    /// accessors).
    pub fn base(&self) -> &AdapterFactoryEditingDomain {
        &self.base
    }

    /// Run `body` as a read transaction (Java `runExclusive`). Read transactions
    /// only read the model; when this thread already holds a write transaction
    /// the call is a plain re-entry, mirroring `ReentrantReadWriteLock`.
    pub fn run_exclusive(&self, body: impl FnOnce()) {
        body();
    }

    /// Run `body` as a write transaction (Java `runExclusive(..., isWrite=true)`).
    /// The outermost transaction defers notifications and flushes them,
    /// coalesced, on commit.
    pub fn run_write(&self, body: impl FnOnce()) {
        let outermost = self.depth.get() == 0;
        if outermost {
            set_delivery_deferred(true);
        }
        self.depth.set(self.depth.get() + 1);

        // Run the body, capturing a panic so the transaction state is always
        // unwound (C++ RAII + try/catch). A panicking body still flushes the
        // notifications accumulated so far, matching the C++ catch path.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));

        self.depth.set(self.depth.get() - 1);
        if self.depth.get() == 0 {
            // Outermost commit: restore direct delivery, then flush coalesced.
            set_delivery_deferred(false);
            flush_deferred_notifications();
        }

        if let Err(payload) = outcome {
            std::panic::resume_unwind(payload);
        }
    }

    /// Whether a transaction is currently open (EMF `isInTransaction`).
    pub fn is_in_transaction(&self) -> bool {
        self.depth.get() > 0
    }

    /// The current nested-transaction depth (EMF transaction depth).
    pub fn transaction_depth(&self) -> i32 {
        self.depth.get()
    }

    /// Whether notifications are delivered immediately (C++
    /// `isDeliverNotifications`).
    pub fn is_deliver_notifications() -> bool {
        !is_delivery_deferred()
    }

    /// Turn deferred delivery on/off (C++ `setDeliverNotifications`).
    pub fn set_deliver_notifications(deliver: bool) {
        set_delivery_deferred(!deliver);
    }

    /// Coalesce and deliver every deferred notification (C++
    /// `flushPendingNotifications`).
    pub fn flush_pending_notifications() {
        flush_deferred_notifications();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use emf_common::eobject::EObject;
    use emf_common::notification::{Adapter, Notification};
    use emf_common::value::Val;
    use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, PackageRegistry};

    use super::*;

    fn registry() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("d");
        pkg.set_ns_prefix("d");
        let mut item = EClass::new("Item", EClassKind::Class);
        let mut name = emf_ecore::EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        item.add_feature(name);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    fn counting_adapter(count: Rc<RefCell<usize>>) -> Box<dyn Adapter> {
        struct Count(Rc<RefCell<usize>>);
        impl Adapter for Count {
            fn notify_changed(&mut self, _n: &Notification) {
                *self.0.borrow_mut() += 1;
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
        Box::new(Count(count))
    }

    #[test]
    fn run_write_defers_then_delivers_on_commit() {
        let reg = registry();
        let obj = Rc::new(RefCell::new(DynamicEObject::new_in(
            reg.find_class("Item").unwrap(),
            reg.clone(),
        )));
        let count = Rc::new(RefCell::new(0usize));
        obj.borrow()
            .add_adapter(counting_adapter(Rc::clone(&count)));

        let domain = TransactionalEditingDomain::new();
        domain.run_write(|| {
            obj.borrow_mut().e_set("name", Val::string("during"));
            assert_eq!(*count.borrow(), 0); // deferred
            assert!(domain.is_in_transaction());
        });
        assert_eq!(*count.borrow(), 1); // flushed once
        assert!(!domain.is_in_transaction());
    }

    #[test]
    fn nested_write_commits_only_at_outermost() {
        let reg = registry();
        let obj = Rc::new(RefCell::new(DynamicEObject::new_in(
            reg.find_class("Item").unwrap(),
            reg.clone(),
        )));
        let count = Rc::new(RefCell::new(0usize));
        obj.borrow()
            .add_adapter(counting_adapter(Rc::clone(&count)));

        let domain = TransactionalEditingDomain::new();
        domain.run_write(|| {
            obj.borrow_mut().e_set("name", Val::string("outer"));
            domain.run_write(|| {
                obj.borrow_mut().e_set("name", Val::string("inner"));
                assert_eq!(*count.borrow(), 0);
            });
            assert_eq!(*count.borrow(), 0); // inner commit did not flush
        });
        assert_eq!(*count.borrow(), 1); // merged to one SET
    }
}
