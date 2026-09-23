//! `EcoreSwitch` — a visitor that dispatches a call on an object's runtime
//! Ecore meta-class (C++ `emf-ecore-util/EcoreSwitch`, aligned to Java
//! `org.eclipse.emf.ecore.util.Switch<T>`).
//!
//! The "switching" model object is any `UserEObject` whose class belongs to
//! the Ecore meta-meta-model (`EClass`, `EAttribute`, `EReference`, ...) as
//! brought up by `crate::ecore_package`. A subclass overrides one or more
//! `case_*` methods; `do_switch` walks the object's class and (through the
//! Ecore inheritance table) the meta-class hierarchy, returning the first
//! matching case result. The base implementations all return `None`, so the
//! default outcome for an unhandled class is "no result".

use std::rc::Rc;

use emf_common::value::ObjectRef;

/// A switch over the Ecore meta-meta-model.
///
/// Implementors override the `case_*` slot(s) they care about and then call
/// [`EcoreSwitch::do_switch`] on a model object. Every case receives the
/// object being switched on and may return any result (the head object itself,
/// a computed value, nothing, ...).
pub trait EcoreSwitch {
    /// Dispatch on `obj`, calling the most specific `case_*` for its class.
    ///
    /// Dispatch order follows the Ecore meta-class hierarchy: the object's own
    /// class first, then its supertypes (`EClass` -> ... -> `EObject`), then
    /// the model-object fallback. The first case that returns `Some` wins.
    fn do_switch(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        let class = obj.borrow().e_class().to_string();
        if let Some(r) = self.dispatch_meta(&class, obj) {
            return Some(r);
        }
        self.case_e_object(obj)
    }

    /// Like [`EcoreSwitch::do_switch`]: returns the raw dispatched meta-case
    /// result, skipping the `case_e_object` fallback.
    fn do_switch_no_default(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        let class = obj.borrow().e_class().to_string();
        self.dispatch_meta(&class, obj)
    }

    /// Route `class` through the meta-case slots. Returns the first non-`None`
    /// result along the inheritance walk.
    fn dispatch_meta(&mut self, class: &str, obj: &ObjectRef) -> Option<ObjectRef> {
        let mut current = class.to_string();
        loop {
            let hit = match current.as_str() {
                "EClass" => self.case_e_class(obj),
                "EAttribute" => self.case_e_attribute(obj),
                "EReference" => self.case_e_reference(obj),
                "EDataType" => self.case_e_data_type(obj),
                "EEnum" => self.case_e_enum(obj),
                "EEnumLiteral" => self.case_e_enum_literal(obj),
                "EOperation" => self.case_e_operation(obj),
                "EParameter" => self.case_e_parameter(obj),
                "EPackage" => self.case_e_package(obj),
                "EFactory" => self.case_e_factory(obj),
                "EAnnotation" => self.case_e_annotation(obj),
                "EStructuralFeature" => self.case_e_structural_feature(obj),
                "EClassifier" => self.case_e_classifier(obj),
                "ETypedElement" => self.case_e_typed_element(obj),
                "ENamedElement" => self.case_e_named_element(obj),
                "EModelElement" => self.case_e_model_element(obj),
                "ETypeParameter" => self.case_e_type_parameter(obj),
                "EGenericType" => self.case_e_generic_type(obj),
                "EObject" => self.case_e_object(obj),
                _ => None,
            };
            if let Some(r) = hit {
                return Some(r);
            }
            // Walk up the (flat) Ecore hierarchy for this meta-class.
            match class_of(current.as_str()) {
                Some(parent) => {
                    if current == parent {
                        return None;
                    }
                    current = parent.to_string();
                }
                None => return None,
            }
        }
    }

    // ---- overridable case slots ----
    fn case_e_class(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_attribute(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_reference(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_data_type(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_enum(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_enum_literal(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_operation(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_parameter(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_package(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_factory(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_annotation(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_structural_feature(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_classifier(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_typed_element(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_named_element(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_model_element(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_type_parameter(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    fn case_e_generic_type(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
    /// The most general case: every model object falls through here when no
    /// more specific meta-case matched.
    fn case_e_object(&mut self, _obj: &ObjectRef) -> Option<ObjectRef> {
        None
    }
}

/// Direct parent of an Ecore meta-class in the flat meta-meta-model. `None`
/// for the model-object root or unknown classes (no further walking).
fn class_of(meta: &str) -> Option<&'static str> {
    Some(match meta {
        "EClass" | "EDataType" | "EEnum" => "EClassifier",
        "EAttribute" | "EReference" => "EStructuralFeature",
        "EClassifier" | "EStructuralFeature" | "ETypeParameter" => "ETypedElement",
        "ETypedElement" | "EOperation" | "EParameter" => "ENamedElement",
        "ENamedElement" | "EAnnotation" => "EModelElement",
        "EModelElement" | "EPackage" | "EFactory" | "EEnumLiteral" => "EObject",
        _ => return None,
    })
}

/// An adapter-friendly default switch whose `case_e_object` echoes the object
/// itself (C++ base `EcoreSwitch::caseEObject` returns the object unchanged).
pub struct DefaultEcoreSwitch {
    seen: Vec<String>,
}

impl EcoreSwitch for DefaultEcoreSwitch {
    fn case_e_object(&mut self, obj: &ObjectRef) -> Option<ObjectRef> {
        self.seen.push(obj.borrow().e_class().to_string());
        Some(Rc::clone(obj))
    }
}

impl DefaultEcoreSwitch {
    /// New echo-style default switch.
    pub fn new() -> Self {
        Self { seen: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use emf_ecore::{EPackage, PackageRegistry};
    use emf_ecore::{make_package_ref, DynNode, DynamicEObject, EClass, EClassKind};

    fn registry() -> PackageRegistry {
        let mut reg = PackageRegistry::new();
        // Seed with the real Ecore package so EClass/EAttribute instantiate.
        reg.register(emf_ecore::ecore_package::ecore_package());
        reg
    }

    fn meta_obj(name: &str) -> ObjectRef {
        let mut cls = EClass::new(name.to_string(), EClassKind::Class);
        let mut nm = emf_ecore::EStructuralFeature::attribute("name");
        nm.set_type_name("EString");
        cls.add_feature(nm);
        let reg = registry();
        Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg)))
    }

    #[test]
    fn dispatches_ec_class() {
        let mut sw = DefaultEcoreSwitch::new();
        let obj = meta_obj("EClass");
        let r = sw.do_switch(&obj);
        assert_eq!(r.map(|o| o.borrow().e_class().to_string()).unwrap(), "EClass");
        assert_eq!(sw.seen, vec!["EClass".to_string()]);
    }

    #[test]
    fn unhandled_class_hits_e_object() {
        struct Noop;
        impl EcoreSwitch for Noop {}
        let mut sw = Noop;
        let obj = meta_obj("EClass");
        assert!(sw.do_switch(&obj).is_none());
    }
}