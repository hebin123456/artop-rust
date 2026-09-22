//! `EcoreUtil.Copier` (EMF `org.eclipse.emf.ecore.util.EcoreUtil$Copier`,
//! C++ `emf-ecore-util/Copier`).
//!
//! Deep-copies an `EObject` graph — attributes by value, containment children
//! recursively, and non-containment (cross) references redirected so they point
//! at the *copies* of their targets rather than the originals. It is a generic
//! EMF utility with no knowledge of any domain metamodel.
//!
//! The copy operates over `DynamicEObject` reflection: the copied object gets
//! the same `EClass` and package registry as the source, so feature storage and
//! serialization behave identically.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emf_common::eobject::downcast_ref;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::DynamicEObject;

use super::ecore_util::ptr;

/// Copies a graph of `DynamicEObject`s, preserving containment and redirecting
/// cross references to copies.
pub struct Copier {
    /// Original object address -> its copy.
    map: HashMap<usize, ObjectRef>,
    /// Copy-in-creation order, so `copy_references` can run and extend the set.
    order: Vec<(ObjectRef, ObjectRef)>,
}

impl Default for Copier {
    fn default() -> Self {
        Self::new()
    }
}

impl Copier {
    /// New, empty copier.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Deep-copy a single root (plus everything it contains / references).
    pub fn copy(&mut self, root: &ObjectRef) -> Result<ObjectRef, String> {
        self.copy_one(root)?;
        self.settle_references()?;
        self.map
            .get(&ptr(root))
            .cloned()
            .ok_or_else(|| "copy of root missing after copy".to_string())
    }

    /// Deep-copy several roots.
    pub fn copy_all(&mut self, roots: &[ObjectRef]) -> Result<Vec<ObjectRef>, String> {
        let mut out = Vec::with_capacity(roots.len());
        for r in roots {
            out.push(self.copy(r)?);
        }
        Ok(out)
    }

    /// The copy of `original`, if it has been copied.
    pub fn get(&self, original: &ObjectRef) -> Option<ObjectRef> {
        self.map.get(&ptr(original)).cloned()
    }

    /// Forget all recorded copies (keeps the copier reusable).
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// Number of objects copied so far.
    pub fn copied_count(&self) -> usize {
        self.map.len()
    }

    /// Copy one object: create a blank same-class copy, then recurse into its
    /// containment children (records the pair *before* recursing to break
    /// cycles in cyclic containment).
    fn copy_one(&mut self, original: &ObjectRef) -> Result<ObjectRef, String> {
        let key = ptr(original);
        if let Some(existing) = self.map.get(&key) {
            return Ok(Rc::clone(existing));
        }

        let (class, registry) = {
            let b = original.borrow();
            let dy = downcast_ref::<DynamicEObject>(&*b)
                .ok_or_else(|| format!("cannot copy non-DynamicEObject '{}'", b.e_class()))?;
            (dy.class().clone(), dy.registry().cloned())
        };

        let copy: ObjectRef = match registry {
            Some(reg) => Rc::new(RefCell::new(DynamicEObject::new_in(class, reg))),
            None => Rc::new(RefCell::new(DynamicEObject::new(class))),
        };
        self.map.insert(key, Rc::clone(&copy));
        self.order.push((Rc::clone(original), Rc::clone(&copy)));

        // Recurse into containment children.
        let containments: Vec<(String, Vec<ObjectRef>, bool)> = {
            let b = original.borrow();
            let dy = downcast_ref::<DynamicEObject>(&*b).expect("downcast just succeeded");
            let mut feats = Vec::new();
            for f in dy.all_containments() {
                let name = f.name().to_string();
                if let Some(val) = dy.e_get_by_name(&name) {
                    feats.push((name, object_refs(&val), f.is_many()));
                }
            }
            feats
        };
        for (name, children, many) in containments {
            if children.is_empty() {
                continue;
            }
            let mut mapped = Vec::with_capacity(children.len());
            for ch in &children {
                mapped.push(self.copy_one(ch)?);
            }
            let copy_obj = Rc::clone(&copy);
            let mut cb = copy_obj.borrow_mut();
            if many || mapped.len() > 1 {
                cb.e_set(&name, val_list(mapped));
            } else {
                cb.e_set(&name, Val::Object(Rc::clone(&mapped[0])));
            }
        }

        // Copy attributes by value (non-reference features live as atomics).
        let attributes: Vec<(String, Val)> = {
            let b = original.borrow();
            let dy = downcast_ref::<DynamicEObject>(&*b).expect("downcast just succeeded");
            let mut feats = Vec::new();
            for f in dy.all_structural_features() {
                if f.is_reference() {
                    continue;
                }
                let name = f.name().to_string();
                if let Some(val) = dy.e_get_by_name(&name) {
                    feats.push((name, val));
                }
            }
            feats
        };
        {
            let mut cb = copy.borrow_mut();
            for (name, val) in attributes {
                if !val.is_null() {
                    cb.e_set(&name, val);
                }
            }
        }
        Ok(copy)
    }

    /// Run reference-fixing to a fixpoint: non-containment references point to
    /// the copies of their targets (creating copies on demand), and newly
    /// created copies get their own references fixed in later rounds.
    fn settle_references(&mut self) -> Result<(), String> {
        loop {
            let n = self.order.len();
            let snapshot = self.order.clone();
            for (orig, copy) in &snapshot {
                self.copy_references(orig, copy)?;
            }
            // If copy_references created new pairs, another round is needed.
            if self.order.len() == n {
                break;
            }
        }
        Ok(())
    }

    /// Redirect a source object's non-containment references on its copy.
    fn copy_references(&mut self, original: &ObjectRef, copy: &ObjectRef) -> Result<(), String> {
        let refs: Vec<(String, bool, Vec<ObjectRef>)> = {
            let b = original.borrow();
            let dy = downcast_ref::<DynamicEObject>(&*b)
                .ok_or_else(|| format!("cannot copy non-DynamicEObject '{}'", b.e_class()))?;
            let mut feats = Vec::new();
            for f in dy.all_references() {
                if f.is_containment() {
                    continue;
                }
                let name = f.name().to_string();
                if let Some(val) = dy.e_get_by_name(&name) {
                    feats.push((name, f.is_many(), object_refs(&val)));
                }
            }
            feats
        };

        for (name, many, targets) in refs {
            if targets.is_empty() {
                continue;
            }
            let mut mapped = Vec::with_capacity(targets.len());
            for t in &targets {
                let c = match self.map.get(&ptr(t)) {
                    Some(c) => Rc::clone(c),
                    None => self.copy_one(t)?, // dangling reference: copy its target too
                };
                mapped.push(c);
            }
            let mut cb = copy.borrow_mut();
            if many || mapped.len() > 1 {
                cb.e_set(&name, val_list(mapped));
            } else {
                cb.e_set(&name, Val::Object(Rc::clone(&mapped[0])));
            }
        }
        Ok(())
    }
}

fn object_refs(v: &Val) -> Vec<ObjectRef> {
    if let Some(o) = v.as_object() {
        vec![Rc::clone(o)]
    } else if let Some(l) = v.as_list() {
        l.iter()
            .filter_map(|x| x.as_object().map(Rc::clone))
            .collect()
    } else {
        Vec::new()
    }
}

fn val_list(objs: Vec<ObjectRef>) -> Val {
    Val::List(objs.into_iter().map(Val::Object).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::{
        make_package_ref, EClass, EClassKind, EPackage, EStructuralFeature, PackageRegistry,
    };

    fn fleet_registry() -> (PackageRegistry, EClass, EClass, EClass) {
        let mut pkg = EPackage::new("fleet");
        pkg.set_ns_prefix("fl");

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
        let mut fleet_name = EStructuralFeature::attribute("name");
        fleet_name.set_type_name("EString");
        let mut vehicles = EStructuralFeature::reference_many("vehicles");
        vehicles.set_type_name("Vehicle");
        vehicles.set_containment(true);
        let mut primary = EStructuralFeature::reference("primary");
        primary.set_type_name("Vehicle");
        // primary is a non-containment cross reference.
        fleet.add_feature(fleet_name);
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

    fn set(obj: &ObjectRef, name: &str, val: Val) {
        obj.borrow_mut().e_set(name, val);
    }
    fn make(cls: EClass, reg: PackageRegistry) -> ObjectRef {
        Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg)))
    }
    fn get_str(obj: &ObjectRef, name: &str) -> String {
        obj.borrow()
            .e_get(name)
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    }
    fn get_objs(obj: &ObjectRef, name: &str) -> Vec<ObjectRef> {
        obj.borrow()
            .e_get(name)
            .map(|v| object_refs(&v))
            .unwrap_or_default()
    }

    #[test]
    fn deep_copy_isomorphic_and_distinct() {
        let (reg, d, v, f) = fleet_registry();
        let fleet = make(f, reg.clone());
        set(&fleet, "name", Val::String("Main".into()));

        let v1 = make(v.clone(), reg.clone());
        set(&v1, "name", Val::String("Car-1".into()));
        let drv = make(d, reg.clone());
        set(&drv, "name", Val::String("Pilot".into()));
        set(&v1, "driver", Val::Object(Rc::clone(&drv)));

        let v2 = make(v, reg);
        set(&v2, "name", Val::String("Car-2".into()));

        set(
            &fleet,
            "vehicles",
            Val::List(vec![
                Val::Object(Rc::clone(&v1)),
                Val::Object(Rc::clone(&v2)),
            ]),
        );
        set(&fleet, "primary", Val::Object(Rc::clone(&v1)));

        let mut copier = Copier::new();
        let copy = copier.copy(&fleet).unwrap();

        // Same class, same attribute values.
        assert_eq!(copy.borrow().e_class(), "Fleet");
        assert_eq!(get_str(&copy, "name"), "Main");

        // Distinct copy of the fleet.
        assert!(!Rc::ptr_eq(&fleet, &copy));

        // Vehicles copied (distinct) but same count & order.
        let veh = get_objs(&copy, "vehicles");
        assert_eq!(veh.len(), 2);
        assert!(!Rc::ptr_eq(&veh[0], &v1));
        assert!(!Rc::ptr_eq(&veh[1], &v2));
        assert_eq!(get_str(&veh[0], "name"), "Car-1");
        assert_eq!(get_str(&veh[1], "name"), "Car-2");

        // Containment child (driver) copied and reachable.
        let drivers = get_objs(&veh[0], "driver");
        assert_eq!(drivers.len(), 1);
        assert!(!Rc::ptr_eq(&drivers[0], &drv));
        assert_eq!(get_str(&drivers[0], "name"), "Pilot");

        // Cross reference `primary` now points at the *copy* of v1, not v1 itself.
        let prim = get_objs(&copy, "primary");
        assert_eq!(prim.len(), 1);
        assert!(!Rc::ptr_eq(&prim[0], &v1));
        // The same object appears both as vehicles[0] and primary -> shared copy.
        assert!(Rc::ptr_eq(&prim[0], &veh[0]));

        // Original graph untouched.
        assert_eq!(get_objs(&fleet, "primary")[0].as_ptr(), v1.as_ptr());
    }

    #[test]
    fn copy_preserves_registry_and_class() {
        let (reg, d, _v, _f) = fleet_registry();
        let driver = make(d, reg.clone());
        set(&driver, "name", Val::String("X".into()));

        let mut copier = Copier::new();
        let copy = copier.copy(&driver).unwrap();
        let b = copy.borrow();
        let dy = downcast_ref::<DynamicEObject>(&*b).unwrap();
        assert_eq!(dy.class().name(), "Driver");
        assert_eq!(get_str(&copy, "name"), "X");
    }
}
