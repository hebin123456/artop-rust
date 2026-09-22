//! `ECrossReferenceAdapter` — collect the non-containment (cross) references
//! of an `EObject` subtree.
//!
//! Port of C++ `emf-ecore-util/ECrossReferenceAdapter` (aligned to Java
//! `org.eclipse.emf.ecore.util.ECrossReferenceAdapter`).
//!
//! In EMF this adapter listens for structural-change notifications to keep an
//! incremental `nonContainmentReferences` set. Our reflection layer currently
//! exposes the reference graph snapshot-wise (see [`emf_ecore::DynamicEObject`]),
//! so the Rust port exposes the same contract — `get_non_containment_references`
//! — backed by an eager scan of the attached subtree. The `attached_` bookkeeping
//! is kept so it interoperates with the notification-based flow when that lands.
//!
//! Everything here is generic EMF and knows nothing about any domain metamodel
//! (see the decoupling principle in `docs/PROGRESS.md`).

use std::collections::HashSet;
use std::rc::Rc;

use emf_common::value::ObjectRef;

use crate::ecore_util::{e_all_contents, e_cross_references};

/// Collects every object reachable by a non-containment reference from any
/// object in the attached subtree.
#[derive(Debug, Default, Clone)]
pub struct ECrossReferenceAdapter {
    /// The roots to which this adapter has been attached.
    attached: Vec<ObjectRef>,
    /// The accumulated non-containment reference targets.
    non_containment_references: HashSet<ObjectRefKey>,
}

/// A stable identity for an object reference (the raw pointer address), since
/// `ObjectRef` implements neither `Hash` nor `Eq` for hashing. Exposed so the
/// non-containment set is usable; treat it as an opaque key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectRefKey(usize);

impl ObjectRefKey {
    /// The identity of an object reference.
    pub fn of(o: &ObjectRef) -> Self {
        Self(Rc::as_ptr(o) as *const () as usize)
    }

    /// The underlying pointer address.
    pub fn as_usize(self) -> usize {
        self.0
    }
}

impl ECrossReferenceAdapter {
    /// A new empty adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach to `e_obj` and its containment subtree, collecting its cross
    /// references. (EMF `addAdapterTo`.)
    pub fn add_adapter_to(&mut self, e_obj: ObjectRef) {
        // Collect non-containment references, then record the root.
        self.non_containment_references = self.collect(e_obj.clone());
        if !self.attached.iter().any(|a| Rc::ptr_eq(a, &e_obj)) {
            self.attached.push(e_obj);
        }
    }

    /// Detach, dropping the collected references. (EMF `removeAdapterFrom`.)
    pub fn remove_adapter_from(&mut self, e_obj: &ObjectRef) {
        self.attached.retain(|a| !Rc::ptr_eq(a, e_obj));
        if self.attached.is_empty() {
            self.non_containment_references.clear();
        } else {
            self.non_containment_references = self.collect_all();
        }
    }

    /// The current non-containment reference target set.
    pub fn get_non_containment_references(&self) -> &HashSet<ObjectRefKey> {
        &self.non_containment_references
    }

    /// Whether `obj` is currently referenced as a non-containment target.
    pub fn contains(&self, obj: &ObjectRef) -> bool {
        self.non_containment_references
            .contains(&ObjectRefKey::of(obj))
    }

    /// The roots this adapter is attached to.
    pub fn attached(&self) -> &[ObjectRef] {
        &self.attached
    }

    /// Collect cross references from a single subtree root.
    fn collect(&self, root: ObjectRef) -> HashSet<ObjectRefKey> {
        let mut out = HashSet::new();
        for cr in e_cross_references(&root) {
            out.insert(ObjectRefKey::of(&cr));
        }
        for child in e_all_contents(&root) {
            for cr in e_cross_references(&child) {
                out.insert(ObjectRefKey::of(&cr));
            }
        }
        out
    }

    /// Recollect across all attached roots.
    fn collect_all(&self) -> HashSet<ObjectRefKey> {
        let mut out = HashSet::new();
        for r in &self.attached {
            for k in self.collect(Rc::clone(r)) {
                out.insert(k);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::value::Val;
    use emf_ecore::{make_package_ref, EClass, EClassKind, EStructuralFeature, PackageRegistry};
    use std::cell::RefCell;

    /// Diagram: `Fleet { parts: Part(*) containment; primary: Part (cross) }`,
    /// `Part { name }`.
    fn diagram() -> (EClass, EClass, PackageRegistry) {
        let mut pkg = emf_ecore::EPackage::new("diag");
        pkg.set_ns_prefix("d");

        let mut part = EClass::new("Part", EClassKind::Class);
        let mut pname = EStructuralFeature::attribute("name");
        pname.set_type_name("EString");
        part.add_feature(pname);

        let mut fleet = EClass::new("Fleet", EClassKind::Class);
        let mut parts = EStructuralFeature::reference_many("parts");
        parts.set_type_name("Part");
        parts.set_containment(true);
        let mut primary = EStructuralFeature::reference("primary");
        primary.set_type_name("Part");
        fleet.add_feature(parts);
        fleet.add_feature(primary);

        pkg.add_class(part);
        pkg.add_class(fleet);
        let mut reg = PackageRegistry::new();
        reg.register(make_package_ref(pkg));
        let p = reg.find_class("Part").unwrap();
        let f = reg.find_class("Fleet").unwrap();
        (p, f, reg)
    }

    fn make(cls: &EClass, reg: &PackageRegistry) -> ObjectRef {
        Rc::new(RefCell::new(emf_ecore::DynamicEObject::new_in(
            cls.clone(),
            reg.clone(),
        )))
    }

    #[test]
    fn collects_non_containment_references() {
        let (part_cls, fleet_cls, reg) = diagram();

        let p1 = make(&part_cls, &reg);
        let p2 = make(&part_cls, &reg);
        p1.borrow_mut().e_set("name", Val::String("P1".into()));
        p2.borrow_mut().e_set("name", Val::String("P2".into()));

        let fleet = make(&fleet_cls, &reg);
        // Two containment parts + one cross reference to p2.
        fleet.borrow_mut().e_set(
            "parts",
            Val::List(vec![
                Val::Object(Rc::clone(&p1)),
                Val::Object(Rc::clone(&p2)),
            ]),
        );
        fleet
            .borrow_mut()
            .e_set("primary", Val::Object(Rc::clone(&p2)));

        let mut adapter = ECrossReferenceAdapter::new();
        adapter.add_adapter_to(Rc::clone(&fleet));

        assert!(
            adapter.contains(&p2),
            "p2 is a cross-reference target via fleet.primary"
        );
        assert!(
            !adapter.contains(&p1),
            "p1 is containment-only and must not be a cross-reference"
        );
        assert_eq!(adapter.attached().len(), 1);
    }

    #[test]
    fn remove_clears_when_empty() {
        let (part_cls, fleet_cls, reg) = diagram();
        let p = make(&part_cls, &reg);
        let fleet = make(&fleet_cls, &reg);
        fleet
            .borrow_mut()
            .e_set("primary", Val::Object(Rc::clone(&p)));

        let mut adapter = ECrossReferenceAdapter::new();
        adapter.add_adapter_to(Rc::clone(&fleet));
        assert!(adapter.contains(&p));

        adapter.remove_adapter_from(&fleet);
        assert_eq!(adapter.get_non_containment_references().len(), 0);
        assert!(adapter.attached().is_empty());
    }
}
