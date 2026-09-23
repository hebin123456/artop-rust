//! Rust port parity tests for `EcoreSwitchTests.cpp` (C++
//! `emf-ecore-util`, aligned to Java `org.eclipse.emf.ecore.util.Switch`).
//!
//! Follows the C++ `CountingSwitch` pattern: a switch subclass records how many
//! times each `case_*` slot fires, then `doSwitch` is called with real
//! EClass / EAttribute / EReference meta-objects.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::value::ObjectRef;
use emf_ecore::{
    make_package_ref, DynNode, DynamicEObject, EClass, EClassKind, EStructuralFeature,
    PackageRegistry,
};
use emf_ecore_util::ecore_switch::EcoreSwitch;

// ---------- helpers ----------

/// A registry seeded with the real Ecore meta-meta-model. This lets us build
/// dynamic objects whose class is one of the Ecore meta-classes.
fn ecore_registry() -> PackageRegistry {
    let mut reg = PackageRegistry::new();
    reg.register(emf_ecore::ecore_package::ecore_package());
    reg
}

/// Build a dynamic meta-object of a given Ecore class name.
fn meta_obj(reg: &PackageRegistry, name: &str) -> ObjectRef {
    let mut cls = EClass::new(name.to_string(), EClassKind::Class);
    let mut nm = EStructuralFeature::attribute("name");
    nm.set_type_name("EString");
    cls.add_feature(nm);
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
}

/// The C++ `CountingSwitch`: tallies each case slot.
struct CountingSwitch {
    e_class_count: usize,
    e_attr_count: usize,
    e_ref_count: usize,
    default_count: usize,
}

impl CountingSwitch {
    fn new() -> Self {
        Self {
            e_class_count: 0,
            e_attr_count: 0,
            e_ref_count: 0,
            default_count: 0,
        }
    }
}

impl EcoreSwitch for CountingSwitch {
    fn case_e_class(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        self.e_class_count += 1;
        Some(Rc::clone(obj))
    }
    fn case_e_attribute(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        self.e_attr_count += 1;
        Some(Rc::clone(obj))
    }
    fn case_e_reference(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        self.e_ref_count += 1;
        Some(Rc::clone(obj))
    }
    fn case_e_object(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        self.default_count += 1;
        Some(Rc::clone(obj))
    }
}

// ---------- tests ----------

#[test]
fn ecore_switch_dispatches_ec_class_e_attribute_e_reference() {
    let reg = registry();
    let cls = meta_obj(&reg, "EClass");
    let attr = meta_obj(&reg, "EAttribute");
    let reff = meta_obj(&reg, "EReference");

    let mut sw = CountingSwitch::new();
    sw.do_switch(&cls);
    sw.do_switch(&attr);
    sw.do_switch(&reff);

    assert_eq!(sw.e_class_count, 1);
    assert_eq!(sw.e_attr_count, 1);
    assert_eq!(sw.e_ref_count, 1);
    assert_eq!(sw.default_count, 0);
}

// ---------- test-only helpers ----------

fn epkg() -> emf_ecore::EPackage {
    let mut ep = emf_ecore::EPackage::new("Ecore");
    ep.set_ns_uri("http://www.eclipse.org/emf/2002/Ecore");
    ep.set_ns_prefix("ecore");
    for meta in [
        "EObject",
        "EModelElement",
        "ENamedElement",
        "ETypedElement",
        "EClassifier",
        "EClass",
        "EDataType",
        "EEnum",
        "EEnumLiteral",
        "EStructuralFeature",
        "EAttribute",
        "EReference",
        "EOperation",
        "EParameter",
        "EPackage",
        "EFactory",
        "ETypeParameter",
        "EGenericType",
        "EAnnotation",
    ] {
        let mut c = EClass::new(meta.to_string(), EClassKind::Class);
        let mut nm = EStructuralFeature::attribute("name");
        nm.set_type_name("EString");
        c.add_feature(nm);
        ep.add_class(c);
    }
    ep
}

fn registry() -> PackageRegistry {
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(epkg()));
    reg
}