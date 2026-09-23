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
use emf_ecore::{datatype, DynamicEObject, EClass, EDataType, EPackage};

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

/// The class name of an object (via the reflective `EObject::e_class`).
fn class_name(obj: &ObjectRef) -> String {
    let b = obj.borrow();
    b.e_class().to_string()
}

/// The class instance identity of a `DynamicEObject` (mirrors C++ `EClass*`).
fn class_identity(obj: &ObjectRef) -> u64 {
    let b = obj.borrow();
    downcast_ref::<DynamicEObject>(&*b)
        .map(|d| d.class().instance_id())
        .unwrap_or(0)
}

/// Deep equality of two objects (EMF `EcoreUtil.equals`). Two `None` are
/// equal; one side missing is not; pointer-identical objects are equal; and
/// otherwise the objects must share a class and every structural-feature
/// value must be value-equal.
pub fn equals(a: Option<&ObjectRef>, b: Option<&ObjectRef>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
        (Some(x), Some(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            if class_identity(x) != class_identity(y) {
                return false;
            }
            // Compare every feature present on either side (names + values).
            let names: Vec<String> = feature_names(x);
            if names != feature_names(y) {
                return false;
            }
            for n in names {
                let vx = x.borrow().e_get(&n);
                let vy = y.borrow().e_get(&n);
                if !equals_value(vx.as_ref(), vy.as_ref()) {
                    return false;
                }
            }
            true
        }
    }
}

/// Value equality (EMF `EcoreUtil.equalsValue`): `None` (unset/`std::any{}`)
/// equals another `None`, differs from anything set, and set values compare
/// structurally (`Val::PartialEq`, object refs by identity).
pub fn equals_value(a: Option<&Val>, b: Option<&Val>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
        (Some(x), Some(y)) => x == y,
    }
}

/// The value of the class's ID-marked attribute, or `""` when the class has no
/// ID attribute or it is unset (EMF `EcoreUtil.getID`).
pub fn get_id(obj: &ObjectRef) -> String {
    match id_feature_name(obj) {
        Some(feat) => match obj.borrow().e_get(&feat) {
            Some(Val::String(s)) => s,
            Some(Val::Null) | None => String::new(),
            Some(v) => v.describe(),
        },
        None => String::new(),
    }
}

/// Set the ID-marked attribute to `id`. Returns false when the class has no ID
/// attribute (EMF `EcoreUtil.setID`).
pub fn set_id(obj: &ObjectRef, id: &str) -> bool {
    match id_feature_name(obj) {
        Some(feat) => obj
            .borrow_mut()
            .e_set(&feat, Val::String(id.to_string())),
        None => false,
    }
}

/// The name of the object's class's ID-marked structural feature, if any.
fn id_feature_name(obj: &ObjectRef) -> Option<String> {
    let b = obj.borrow();
    let dy = downcast_ref::<DynamicEObject>(&*b)?;
    dy.all_structural_features()
        .iter()
        .find(|f| f.is_id())
        .map(|f| f.name().to_string())
}

/// The `urn:emf`-style URI of a root object with no resource (EMF
/// `EcoreUtil.getURI`). An object with a container walks up to its root. For a
/// resource-less root the URI is a `urn:emf:///...` path fragment.
pub fn get_uri(obj: &ObjectRef) -> String {
    let root = get_root_container(obj);
    format!("urn:emf://{}", uri_fragment(&root))
}

/// A `//@feat.idx`-style fragment for the object as seen from its root.
fn uri_fragment(obj: &ObjectRef) -> String {
    let root = get_root_container(obj);
    if Rc::ptr_eq(&root, obj) {
        return format!("/{}", class_name(obj));
    }
    // Walk from the root down, matching the object by pointer.
    fn find(node: &ObjectRef, target: &ObjectRef, prefix: &str) -> Option<String> {
        if Rc::ptr_eq(node, target) {
            return Some(prefix.to_string());
        }
        for f in structural_features(node) {
            if !f.is_containment() {
                continue;
            }
            let v = node.borrow().e_get(f.name());
            if let Some(val) = &v {
                if let Some(o) = single_object(val) {
                    if let Some(p) = find(&o, target, &format!("{}@{}", prefix, f.name())) {
                        return Some(p);
                    }
                } else if let Some(l) = list(val) {
                    for (i, item) in l.iter().enumerate() {
                        if let Some(o) = item {
                            if let Some(p) =
                                find(o, target, &format!("{}@{}.{}", prefix, f.name(), i))
                            {
                                return Some(p);
                            }
                        }
                    }
                }
            }
        }
        None
    }
    find(&root, obj, "").unwrap_or_else(|| format!("/{}", class_name(obj)))
}

/// Whether `ancestor` is `obj`'s class or one of its supertypes (EMF
/// `EcoreUtil.isAncestor(EClass, EObject)`; distinct from the object-containment
/// `is_ancestor` above).
pub fn is_ancestor_of(ancestor: &EClass, obj: &ObjectRef) -> bool {
    let b = obj.borrow();
    let dy = match downcast_ref::<DynamicEObject>(&*b) {
        Some(d) => d,
        None => return false,
    };
    if dy.class().name() == ancestor.name() {
        return true;
    }
    let reg = dy
        .registry()
        .cloned()
        .unwrap_or_else(emf_ecore::ecore_package::global);
    dy.class().is_super_type_of(ancestor.name(), &reg)
}

/// Look up an `EDataType` classifier by name inside `pkg` (EMF
/// `EcoreUtil.getEClassifier`). Returns `None` when missing or when the
/// classifier is not a data type (an `EClass`, etc.).
pub fn get_e_classifier(pkg: &EPackage, name: &str) -> Option<EDataType> {
    pkg.find_data_type(name).cloned()
}

/// Parse `literal` into a [`Val`] using the named Ecore built-in data type
/// (EMF `EcoreUtil.createFromString`).
pub fn create_from_string(data_type: &str, literal: &str) -> Val {
    datatype::from_string(data_type, literal)
}

/// Render `value` back to its literal string for the named Ecore built-in data
/// type (EMF `EcoreUtil.convertToString`).
pub fn convert_to_string(data_type: &str, value: &Val) -> String {
    datatype::to_string(data_type, value)
}

/// The structural-feature names of a `DynamicEObject`, own + inherited.
fn feature_names(obj: &ObjectRef) -> Vec<String> {
    structural_features(obj)
        .iter()
        .map(|f| f.name().to_string())
        .collect()
}

/// All structural features (own + inherited) of a `DynamicEObject`.
fn structural_features(obj: &ObjectRef) -> Vec<emf_ecore::EStructuralFeature> {
    let b = obj.borrow();
    match downcast_ref::<DynamicEObject>(&*b) {
        Some(d) => d.all_structural_features(),
        None => Vec::new(),
    }
}

/// The single object pointed at by a `Val`, if any.
fn single_object(v: &Val) -> Option<ObjectRef> {
    v.as_object().cloned()
}

/// The object refs held by a `Val::List`, if any.
fn list(v: &Val) -> Option<Vec<Option<ObjectRef>>> {
    v.as_list().map(|l| l.iter().map(|x| x.as_object().cloned()).collect())
}

/// Object identity key: the raw `Rc` pointer address.
pub(crate) fn ptr(obj: &ObjectRef) -> usize {
    Rc::as_ptr(obj) as *const () as usize
}

// ---- structural mutation / copy / proxy resolution (C++ EcoreUtil) ----

/// Detach `obj` from its container. If the holding feature is single-valued it
/// is unset; if multi-valued `obj` is removed from the list. Returns `false`
/// when `obj` has no container (or its container is not reflective).
pub fn remove(obj: &ObjectRef) -> bool {
    let Some(container) = obj.borrow().e_container() else {
        return false;
    };
    let key = ptr(obj);
    let hit: Option<(String, bool)> = {
        let c = container.borrow();
        let dy = match downcast_ref::<DynamicEObject>(&*c) {
            Some(d) => d,
            None => return false,
        };
        dy.all_containments().into_iter().find_map(|f| {
            let name = f.name().to_string();
            let val = dy.e_get_by_name(&name)?;
            if let Some(o) = val.as_object() {
                (ptr(o) == key && !f.is_many()).then_some((name, false))
            } else if let Some(l) = val.as_list() {
                l.iter()
                    .any(|v| v.as_object().map(|o| ptr(o) == key).unwrap_or(false))
                    .then_some((name, true))
            } else {
                None
            }
        })
    };
    let (name, many) = match hit {
        Some(h) => h,
        None => return false,
    };
    {
        let mut d = container.borrow_mut();
        if many {
            let cur = d.e_get(&name).unwrap_or(Val::List(Vec::new()));
            let kept: Vec<Val> = cur
                .as_list()
                .map(|l| {
                    l.iter()
                        .filter(|v| v.as_object().map(|o| ptr(o) != key).unwrap_or(true))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            d.e_set(&name, Val::List(kept));
        } else {
            d.e_unset(&name);
        }
    }
    obj.borrow_mut().clear_container();
    true
}

/// A structural copy of `obj` (attributes by value, containment children
/// deep-copied, cross references redirected to copies), or `None` on failure.
/// EMF `EcoreUtil.copy`.
pub fn copy(obj: &ObjectRef) -> Option<ObjectRef> {
    let mut copier = crate::copier::Copier::new();
    match copier.copy(obj) {
        Ok(cp) => {
            copier.copy_references().ok()?;
            Some(cp)
        }
        Err(_) => None,
    }
}

/// `EcoreUtil.copyAll`: structural copies of each root.
pub fn copy_all(roots: &[ObjectRef]) -> Vec<ObjectRef> {
    let mut copier = crate::copier::Copier::new();
    match copier.copy_all(roots) {
        Ok(copies) => {
            let _ = copier.copy_references();
            copies
        }
        Err(_) => Vec::new(),
    }
}

/// Resolve a (possibly proxy) object. A `None` stays `None`; a non-proxy is
/// returned as-is; a proxy with no available `ResourceSet` to resolve through
/// is returned as-is too (EMF behaviour: it cannot be resolved).
pub fn resolve(obj: Option<&ObjectRef>) -> Option<ObjectRef> {
    obj.map(Rc::clone)
}

/// `EcoreUtil.resolveAll`: resolve every resolvable object in `obj`'s subtree.
/// With no `ResourceSet`, this is a no-op traversal (proxies are left as-is).
pub fn resolve_all(obj: &ObjectRef) {
    for o in e_all_contents(obj) {
        let _ = o.borrow().e_is_proxy();
    }
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
