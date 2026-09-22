//! `EcoreUtil` static helpers, ported from C++ `emf-ecore-util/EcoreUtil`
//! (aligned to Java `org.eclipse.emf.ecore.util.EcoreUtil`).
//!
//! The helpers here are *generic EMF*: they navigate the containment tree and
//! the cross-reference graph purely through `EObject` reflection, with a fast
//! path over `DynamicEObject` (see the decoupling principle in
//! `docs/PROGRESS.md` — nothing here knows about any domain metamodel).
//!
//! Provided:
//! - `e_contents` / `e_all_contents`: the direct / transitive containment tree;
//! - `e_cross_references`: the direct non-containment (cross) references;
//! - `get_all_contents`: the transitive contents of a set of roots;
//! - `is_ancestor` / `get_root_container`: containment-tree predicates.

use std::cell::Ref;
use std::rc::Rc;

use emf_common::eobject::{downcast_ref, EObject};
use emf_common::value::{ObjectRef, Val};
use emf_ecore::DynamicEObject;

/// The direct containment children of `obj`, in feature / element order.
///
/// Uses the reflective `DynamicEObject::all_containments` fast path when the
/// object is a `DynamicEObject`; otherwise falls back to `EObject::e_contents`.
pub fn e_contents(obj: &ObjectRef) -> Vec<ObjectRef> {
    let b: Ref<'_, dyn EObject> = obj.borrow();
    if let Some(dy) = downcast_ref::<DynamicEObject>(&*b) {
        let mut out = Vec::new();
        for f in dy.all_containments() {
            if let Some(val) = dy.e_get_by_name(f.name()) {
                append_object_refs(&val, &mut out);
            }
        }
        out
    } else {
        b.e_contents()
    }
}

/// The transitive contents of `obj`: every descendant object in the
/// containment tree, in pre-order (a parent before its children).
pub fn e_all_contents(root: &ObjectRef) -> Vec<ObjectRef> {
    fn visit(node: &ObjectRef, out: &mut Vec<ObjectRef>) {
        for child in e_contents(node) {
            out.push(Rc::clone(&child));
            visit(&child, out);
        }
    }
    let mut out = Vec::new();
    visit(root, &mut out);
    out
}

/// The direct cross references of `obj`: objects reachable through
/// non-containment reference features (EMF `EObject.eCrossReferences`).
pub fn e_cross_references(obj: &ObjectRef) -> Vec<ObjectRef> {
    let b: Ref<'_, dyn EObject> = obj.borrow();
    if let Some(dy) = downcast_ref::<DynamicEObject>(&*b) {
        let mut out = Vec::new();
        for f in dy.all_references() {
            if f.is_containment() {
                continue;
            }
            if let Some(val) = dy.e_get_by_name(f.name()) {
                append_object_refs(&val, &mut out);
            }
        }
        out
    } else {
        b.e_cross_references()
    }
}

/// The transitive contents of a set of roots (each root's descendants, not the
/// roots themselves). Deduplicated so an object reachable through multiple
/// roots is reported once.
pub fn get_all_contents(roots: &[ObjectRef]) -> Vec<ObjectRef> {
    let mut out: Vec<ObjectRef> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    for r in roots {
        let mut stack = e_contents(r);
        while let Some(node) = stack.pop() {
            let key = ptr(&node);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.push(Rc::clone(&node));
            for child in e_contents(&node) {
                stack.push(child);
            }
        }
    }
    out
}

/// Whether `ancestor` strictly contains `descendant` in the containment tree.
/// Self is never considered its own ancestor.
pub fn is_ancestor(ancestor: &ObjectRef, descendant: &ObjectRef) -> bool {
    if Rc::ptr_eq(ancestor, descendant) {
        return false;
    }
    e_all_contents(ancestor)
        .iter()
        .any(|n| Rc::ptr_eq(n, descendant))
}

/// Walk up the containment chain to the outermost root. If the object has no
/// tracked container (e.g. a bare `DynamicEObject` that never got a parent),
/// it is its own root.
pub fn get_root_container(obj: &ObjectRef) -> ObjectRef {
    let mut current = Rc::clone(obj);
    loop {
        let up = {
            let b = current.borrow();
            b.e_container()
        };
        match up {
            Some(parent) => current = parent,
            None => return current,
        }
    }
}

/// The nearest object pointing to `obj` from within `scope`, if any.
/// (EMF `EcoreUtil` does not ship an exact `getObjectOfType`; this helper
/// implements "which root/object references this object" by scanning.)
fn append_object_refs(v: &Val, out: &mut Vec<ObjectRef>) {
    if let Some(o) = v.as_object() {
        out.push(Rc::clone(o));
    } else if let Some(l) = v.as_list() {
        for x in l {
            if let Some(o) = x.as_object() {
                out.push(Rc::clone(o));
            }
        }
    }
}

/// Object identity key: the raw `Rc` pointer address.
pub(crate) fn ptr(obj: &ObjectRef) -> usize {
    Rc::as_ptr(obj) as *const () as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::{
        make_package_ref, EClass, EClassKind, EPackage, EStructuralFeature, PackageRegistry,
    };

    /// A tiny "plant" metamodel: `Fleet { vehicles: Vehicle(*) }`,
    /// `Vehicle { name; driver: Driver (contained) }`, `Driver { name }`.
    /// `Vehicle.driver` is a single containment; a non-containment reference
    /// `Fleet.primary` points at a `Vehicle` (a cross reference).
    fn plant_registry() -> (PackageRegistry, EClass, EClass, EClass) {
        let mut pkg = EPackage::new("plant");
        pkg.set_ns_prefix("pl");

        let mut driver = EClass::new("Driver", EClassKind::Class);
        let mut dname = EStructuralFeature::attribute("name");
        dname.set_type_name("EString");
        driver.add_feature(dname);

        let mut vehicle = EClass::new("Vehicle", EClassKind::Class);
        let mut vname = EStructuralFeature::attribute("name");
        vname.set_type_name("EString");
        let mut driver_ref = EStructuralFeature::reference("driver");
        driver_ref.set_type_name("Driver");
        driver_ref.set_containment(true);
        vehicle.add_feature(vname);
        vehicle.add_feature(driver_ref);

        let mut fleet = EClass::new("Fleet", EClassKind::Class);
        let mut vehicles = EStructuralFeature::reference_many("vehicles");
        vehicles.set_type_name("Vehicle");
        vehicles.set_containment(true);
        let mut primary = EStructuralFeature::reference("primary");
        primary.set_type_name("Vehicle");
        fleet.add_feature(vehicles);
        fleet.add_feature(primary);

        pkg.add_class(driver);
        pkg.add_class(vehicle);
        pkg.add_class(fleet);

        let mut reg = PackageRegistry::new();
        reg.register(make_package_ref(pkg));
        let d = reg.find_class("Driver").unwrap();
        let v = reg.find_class("Vehicle").unwrap();
        let f = reg.find_class("Fleet").unwrap();
        (reg, d, v, f)
    }

    fn make(name: &str, value: &str, cls: EClass, reg: PackageRegistry) -> ObjectRef {
        let o = Rc::new(std::cell::RefCell::new(DynamicEObject::new_in(cls, reg)));
        o.borrow_mut().e_set(name, Val::String(value.into()));
        o
    }

    #[test]
    fn direct_and_transitive_contents() {
        let (reg, d, v, _f) = plant_registry();
        let driver = make("name", "Pilot", d, reg.clone());
        let vehicle = make("name", "Car", v, reg);
        vehicle
            .borrow_mut()
            .e_set("driver", Val::Object(Rc::clone(&driver)));

        let direct = e_contents(&vehicle);
        assert_eq!(direct.len(), 1);
        assert!(Rc::ptr_eq(&direct[0], &driver));

        let all = e_all_contents(&vehicle);
        assert_eq!(all.len(), 1);
        assert!(Rc::ptr_eq(&all[0], &driver));
    }

    #[test]
    fn cross_references_exclude_containment() {
        let (reg, d, v, f) = plant_registry();
        let driver = make("name", "Pilot", d, reg.clone());
        let vehicle = make("name", "Car", v, reg.clone());
        vehicle
            .borrow_mut()
            .e_set("driver", Val::Object(Rc::clone(&driver)));

        let fleet = make("name", "Fleet", f, reg);
        // primary is a non-containment cross reference to `vehicle`.
        fleet
            .borrow_mut()
            .e_set("primary", Val::Object(Rc::clone(&vehicle)));

        let cross = e_cross_references(&fleet);
        assert_eq!(cross.len(), 1, "fleet.primary is a cross reference");
        assert!(Rc::ptr_eq(&cross[0], &vehicle));
    }

    #[test]
    fn is_ancestor_checks_containment_chain() {
        let (reg, d, v, _f) = plant_registry();
        let driver = make("name", "Pilot", d, reg.clone());
        let vehicle = make("name", "Car", v, reg);
        vehicle
            .borrow_mut()
            .e_set("driver", Val::Object(Rc::clone(&driver)));

        assert!(is_ancestor(&vehicle, &driver));
        assert!(!is_ancestor(&vehicle, &vehicle));
        assert!(!is_ancestor(&driver, &vehicle));
    }
}
