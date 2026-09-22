//! Shared internal helpers for the compare engines (feature iteration over the
//! reflection surface, value equality, object-list extraction).

use emf_common::eobject::downcast_ref;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::DynamicEObject;

/// The object key for hash-map identity: the `Rc` data pointer address.
pub(crate) fn key(o: &ObjectRef) -> usize {
    std::rc::Rc::as_ptr(o) as *const () as usize
}

/// Pointer identity.
pub(crate) fn is_same(a: &ObjectRef, b: &ObjectRef) -> bool {
    std::rc::Rc::ptr_eq(a, b)
}

/// All structural features (own + inherited) of a `DynamicEObject`; empty for
/// any other (non-dynamic) model object.
pub(crate) fn all_features(o: &ObjectRef) -> Vec<emf_ecore::EStructuralFeature> {
    let b = o.borrow();
    match downcast_ref::<DynamicEObject>(&*b) {
        Some(dy) => dy.all_structural_features(),
        None => Vec::new(),
    }
}

/// The containment children of an object (`e_contents`).
pub(crate) fn contents(o: &ObjectRef) -> Vec<ObjectRef> {
    o.borrow().e_contents()
}

/// Whether two `Val`s are "equal value" (aligned to `EcoreUtil.equalsValue`).
/// Object values compare by pointer identity; lists compare structurally.
pub(crate) fn value_equal(a: &Val, b: &Val) -> bool {
    a == b
}

/// Extract the list of referenced objects from a `Val` (single object or list).
pub(crate) fn object_list(v: &Val) -> Vec<ObjectRef> {
    match v {
        Val::Object(o) => vec![o.clone()],
        Val::List(items) => items
            .iter()
            .filter_map(|x| match x {
                Val::Object(o) => Some(o.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract a single object from a `Val` (only when it is exactly one object).
pub(crate) fn single_object(v: &Val) -> Option<ObjectRef> {
    match v {
        Val::Object(o) => Some(o.clone()),
        _ => None,
    }
}
