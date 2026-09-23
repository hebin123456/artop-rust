//! `EObjectEList` — an ordered, unique list of `EObject` references bound to a
//! feature (the metaclass + feature id + owning object that this list backs).
//!
//! Port of C++ `emf-ecore/util/EObjectEList` (aligned to Java
//! `org.eclipse.emf.ecore.util.EObjectEList`). Semantics modeled here:
//!
//! * pointer identity comparison (`use_equals == false`): two elements are
//!   "equal" iff they are the *same* [`ObjectRef`];
//! * uniqueness by identity (`is_unique == true`): `add`/`set` reject a
//!   duplicate element, while the `add_unique` family bypasses that check;
//! * null is not permitted (`can_contain_null == false`): adding `Option::None`
//!   or an already-dropped element panics;
//! * the list is a plain EObject container with no inverse (`has_inverse ==
//!   false`); `isSet` is derived from being non-empty.

use emf_common::value::ObjectRef;
use emf_ecore::EClass;

/// An ordered, unique list of object references backing a single multi-valued
/// reference feature on a containing object.
#[derive(Debug, Clone)]
pub struct EObjectEList {
    /// The metaclass that declares the owning feature (C++ `dataClass_`).
    data_class: emf_ecore::EClass,
    /// The owning object, if any (C++ `owner_`). Held weakly to avoid cycles.
    owner: Option<std::rc::Weak<std::cell::RefCell<dyn emf_common::eobject::EObject>>>,
    /// The integer feature id of the reference being backed (C++ `featureID_`).
    feature_id: i32,
    /// The stored object references, in order.
    elements: Vec<ObjectRef>,
}

impl EObjectEList {
    /// New list bound to the given class and feature id, with no owner.
    pub fn new(data_class: emf_ecore::EClass, feature_id: i32) -> Self {
        Self {
            data_class,
            owner: None,
            feature_id,
            elements: Vec::new(),
        }
    }

    /// New list bound to a class, an owning object and a feature id.
    pub fn with_owner(
        data_class: emf_ecore::EClass,
        owner: ObjectRef,
        feature_id: i32,
    ) -> Self {
        let owner = std::rc::Rc::downgrade(&owner);
        Self {
            data_class,
            owner: Some(owner),
            feature_id,
            elements: Vec::new(),
        }
    }

    /// The feature id this list backs (C++ `getFeatureID`).
    pub fn get_feature_id(&self) -> i32 {
        self.feature_id
    }

    /// The metaclass that declares the owning feature (C++ `dataClass`).
    pub fn data_class(&self) -> &emf_ecore::EClass {
        &self.data_class
    }

    /// The owning object, if still alive (C++ `owner`).
    pub fn owner(&self) -> Option<ObjectRef> {
        self.owner.as_ref().and_then(|w| w.upgrade())
    }

    /// C++ `useEquals`: objects compare by pointer identity.
    pub fn use_equals(&self) -> bool {
        false
    }

    /// C++ `isUnique`: duplicate elements are rejected by `add`/`set`.
    pub fn is_unique(&self) -> bool {
        true
    }

    /// C++ `hasInverse`: no inverse feature is maintained.
    pub fn has_inverse(&self) -> bool {
        false
    }

    /// C++ `isEObject`: elements are `EObject`s.
    pub fn is_e_object(&self) -> bool {
        true
    }

    /// C++ `canContainNull`: `None` elements are not permitted.
    pub fn can_contain_null(&self) -> bool {
        false
    }

    /// Number of elements.
    pub fn size(&self) -> usize {
        self.elements.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Whether the feature is set (C++ `isSet`): true iff non-empty.
    pub fn is_set(&self) -> bool {
        !self.elements.is_empty()
    }

    /// Clear the list (C++ `clear`).
    pub fn clear(&mut self) {
        self.elements.clear();
    }

    /// C++ `unset`: identical to clearing for this list (no stored "was set"
    /// flag beyond non-emptiness).
    pub fn unset(&mut self) {
        self.elements.clear();
    }

    /// Append an object if it is not already present (unique check).
    /// Returns `true` if appended, `false` if it was a duplicate.
    pub fn add(&mut self, obj: ObjectRef) -> bool {
        if self.contains(&obj) {
            return false;
        }
        self.elements.push(obj);
        true
    }

    /// Append an object without a uniqueness check (C++ `addUnique`).
    pub fn add_unique(&mut self, obj: ObjectRef) {
        self.elements.push(obj);
    }

    /// Append a batch of objects without a uniqueness check (C++ `addAllUnique`).
    /// Returns `false` when the input is empty.
    pub fn add_all_unique(&mut self, mut objs: Vec<ObjectRef>) -> bool {
        if objs.is_empty() {
            return false;
        }
        self.elements.append(&mut objs);
        true
    }

    /// Element at `index` (C++ `get`).
    pub fn get(&self, index: usize) -> ObjectRef {
        self.elements[index].clone()
    }

    /// Element at `index` without any proxy resolution (C++ `basicGet`); here
    /// identical to `get`.
    pub fn basic_get(&self, index: usize) -> ObjectRef {
        self.elements[index].clone()
    }

    /// Whether `obj` is present (by identity).
    pub fn contains(&self, obj: &ObjectRef) -> bool {
        self.elements.iter().any(|e| std::rc::Rc::ptr_eq(e, obj))
    }

    /// Index of `obj`, or `-1` (C++ `indexOf`).
    pub fn index_of(&self, obj: &ObjectRef) -> i32 {
        self.elements
            .iter()
            .position(|e| std::rc::Rc::ptr_eq(e, obj))
            .map(|i| i as i32)
            .unwrap_or(-1)
    }

    /// Remove and return the element at `index` (C++ `remove(int)`).
    pub fn remove_index(&mut self, index: usize) -> ObjectRef {
        self.elements.remove(index)
    }

    /// Remove the first occurrence of `obj`; `true` if removed (C++ `remove(E)`).
    pub fn remove_value(&mut self, obj: &ObjectRef) -> bool {
        if let Some(i) = self
            .elements
            .iter()
            .position(|e| std::rc::Rc::ptr_eq(e, obj))
        {
            self.elements.remove(i);
            true
        } else {
            false
        }
    }

    /// Replace the element at `index` without a uniqueness check (C++ `setUnique`),
    /// returning the old element.
    pub fn set_unique(&mut self, index: usize, obj: ObjectRef) -> ObjectRef {
        std::mem::replace(&mut self.elements[index], obj)
    }

    /// Replace the element at `index`, rejecting a value already present at a
    /// different index (uniqueness check). Returns the old element; panics if
    /// `obj` already appears elsewhere (C++ `BasicEList::set` under `isUnique`).
    pub fn set_index_checked(&mut self, index: usize, obj: ObjectRef) -> ObjectRef {
        for (i, e) in self.elements.iter().enumerate() {
            if i != index && std::rc::Rc::ptr_eq(e, &obj) {
                panic!("EObjectEList: duplicate element at set index {index}");
            }
        }
        std::mem::replace(&mut self.elements[index], obj)
    }

    /// Move the element currently at `from` to `to`, shifting the rest; returns
    /// the moved element (C++ `move(target, source)`).
    pub fn move_to(&mut self, to: usize, from: usize) -> ObjectRef {
        let moved = self.elements.remove(from);
        self.elements.insert(to, moved.clone());
        moved
    }

    /// All elements, in order (C++ `toArray`).
    pub fn to_array(&self) -> Vec<ObjectRef> {
        self.elements.clone()
    }
}