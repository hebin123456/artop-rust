//! The `EObject` trait (C++ `emf-common/EObject`, aligned to EMF `EObject`).
//!
//! In Rust we model object identity with [`crate::value::ObjectRef`]
//! (`Rc<RefCell<dyn EObject>>`). The trait exposes the EMF reflection surface:
//! - `e_class()`: classifier name (inheritance is *data*, in `autosar448-model`);
//! - `e_get` / `e_set` / `e_is_set` / `e_unset`: feature access by id/name;
//! - `e_contents` / `e_container`: the containment tree;
//! - `e_resource`: the owning resource.

use crate::feature_map::FeatureMap;
use crate::value::{ObjectRef, Val};

/// Downcast helper: turn an `&dyn EObject` into a concrete `&T`.
pub fn downcast_ref<T: EObject + 'static>(obj: &dyn EObject) -> Option<&T> {
    obj.as_any().downcast_ref::<T>()
}

/// The base trait for all model objects.
pub trait EObject: std::fmt::Debug {
    /// The classifier name of this object, e.g. `"SwcImplementation"`.
    fn e_class(&self) -> &str;

    /// Structural `as_any`, enabling downcast to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// The owning resource (if any), as a type-erased identifier.
    fn e_resource(&self) -> Option<ObjectRef> {
        None
    }

    /// The direct container object (if any).
    fn e_container(&self) -> Option<ObjectRef> {
        None
    }

    /// Child objects held by containment features.
    fn e_contents(&self) -> Vec<ObjectRef> {
        Vec::new()
    }

    /// Cross references (non-containment references to other resources).
    fn e_cross_references(&self) -> Vec<ObjectRef> {
        Vec::new()
    }

    /// Whether this object is a proxy (unresolved reference).
    fn e_is_proxy(&self) -> bool {
        false
    }

    /// Reflectively read a feature by id.
    fn e_get(&self, feature_id: &str) -> Option<Val> {
        let _ = feature_id;
        None
    }

    /// Reflectively write a feature by id.
    fn e_set(&mut self, feature_id: &str, value: Val) -> bool {
        let _ = (feature_id, value);
        false
    }

    /// Whether a feature is set.
    fn e_is_set(&self, feature_id: &str) -> bool {
        let _ = feature_id;
        false
    }

    /// Unset a feature, restoring its default.
    fn e_unset(&mut self, feature_id: &str) -> bool {
        let _ = feature_id;
        false
    }

    /// The mixed feature map backing volatile/lazy features, if any.
    fn feature_map(&self) -> &'static FeatureMap {
        // A single leaked empty map, shared by all default impls at runtime.
        fn empty() -> Box<FeatureMap> {
            Box::new(FeatureMap::new())
        }
        Box::leak(empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Val;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Foo {
        short_name: Option<String>,
    }

    impl std::fmt::Debug for Foo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Foo")
                .field("short_name", &self.short_name)
                .finish()
        }
    }

    impl EObject for Foo {
        fn e_class(&self) -> &str {
            "Foo"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn e_get(&self, feature_id: &str) -> Option<Val> {
            match feature_id {
                "shortName" => self.short_name.clone().map(Val::String),
                _ => None,
            }
        }
        fn e_set(&mut self, feature_id: &str, value: Val) -> bool {
            if feature_id == "shortName" {
                self.short_name = value.as_str().map(String::from);
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn eobject_reflects_features() {
        let mut foo = Foo { short_name: None };
        assert!(foo.e_set("shortName", Val::String("abc".into())));
        assert_eq!(
            foo.e_get("shortName")
                .and_then(|v| v.as_str().map(String::from)),
            Some("abc".into())
        );
        assert_eq!(foo.e_class(), "Foo");
        assert!(downcast_ref::<Foo>(&foo).is_some());
    }

    #[test]
    fn object_ref_wraps_trait() {
        let rc: ObjectRef = Rc::new(RefCell::new(Foo {
            short_name: Some("x".into()),
        }));
        let c = rc.borrow().e_class().to_string();
        assert_eq!(c, "Foo");
    }
}
