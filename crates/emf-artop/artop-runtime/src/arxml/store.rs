//! Per-object ARXML side tables shared by the loader and the saver.
//!
//! Port of the file-scope stores in the C++ `AutosarXMLLoader` / `AutosarXMLSaver`
//! (`mixedTextStore`, `commentStore`, `mixedContentStore`, `refIsDefaultStore`,
//! `refDestStore`). The C++ keeps these in `EObject*`-keyed maps because its
//! generated classes have a fixed ABI it cannot extend; Rust models the same
//! side data here, keyed by object identity.
//!
//! Layout information (original indentation/comments) is not part of the AUTOSAR
//! metamodel, yet a faithful round-trip must reproduce it. These tables carry
//! that information across the load/save pair.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emf_common::value::ObjectRef;

use crate::arxml::dom;

/// An entry in an object's ordered mixed-content sequence (C++
/// `MixedContentEntry`): the original document order of text, comments and
/// child elements is preserved so the saver can emit it verbatim.
#[derive(Debug, Clone)]
pub enum MixedEntry {
    /// Character data (whitespace included).
    Text(String),
    /// A comment's inner text.
    Comment(String),
    /// A child object (containment or reference target/proxy).
    Element(ObjectRef),
}

/// The identity key of an object handle (C++ uses the raw `EObject*`).
pub fn object_key(o: &ObjectRef) -> usize {
    Rc::as_ptr(o) as *const () as usize
}

thread_local! {
    static MIXED_TEXT: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
    static COMMENTS: RefCell<HashMap<usize, Vec<String>>> = RefCell::new(HashMap::new());
    static MIXED_CONTENT: RefCell<HashMap<usize, Vec<MixedEntry>>> = RefCell::new(HashMap::new());
    static REF_IS_DEFAULT: RefCell<HashMap<usize, bool>> = RefCell::new(HashMap::new());
    /// `(owner, feature, target) -> original DEST`, mirroring the C++
    /// `refDestStore`'s "owner:ref:target" key (the original `DEST` may name an
    /// abstract base class and must be preserved rather than recomputed).
    static REF_DEST: RefCell<HashMap<(usize, String, usize), String>> = RefCell::new(HashMap::new());
    /// `owner -> unmapped XML elements`, mirroring the C++ `unknownContents_`
    /// map populated under `OPTION_RECORD_UNKNOWN_FEATURE`: elements that map to
    /// no feature are replayed verbatim by the saver.
    static UNKNOWN_CONTENT: RefCell<HashMap<usize, Vec<dom::Element>>> =
        RefCell::new(HashMap::new());
}

/// Record the mixed-content text captured for `obj`.
pub fn set_mixed_text(obj: &ObjectRef, text: impl Into<String>) {
    MIXED_TEXT.with(|m| m.borrow_mut().insert(object_key(obj), text.into()));
}

/// The mixed-content text captured for `obj`, if any.
pub fn mixed_text(obj: &ObjectRef) -> Option<String> {
    MIXED_TEXT.with(|m| m.borrow().get(&object_key(obj)).cloned())
}

/// Record the leading comments of `obj`.
pub fn set_comments(obj: &ObjectRef, comments: Vec<String>) {
    COMMENTS.with(|m| m.borrow_mut().insert(object_key(obj), comments));
}

/// The leading comments of `obj`, if any.
pub fn comments(obj: &ObjectRef) -> Option<Vec<String>> {
    COMMENTS.with(|m| m.borrow().get(&object_key(obj)).cloned())
}

/// Append one entry to `obj`'s ordered mixed-content sequence.
pub fn push_mixed_content(obj: &ObjectRef, entry: MixedEntry) {
    MIXED_CONTENT.with(|m| {
        m.borrow_mut()
            .entry(object_key(obj))
            .or_default()
            .push(entry)
    });
}

/// `obj`'s ordered mixed-content sequence, if any.
pub fn mixed_content(obj: &ObjectRef) -> Option<Vec<MixedEntry>> {
    MIXED_CONTENT.with(|m| m.borrow().get(&object_key(obj)).cloned())
}

/// Record whether a resolved reference's `BASE` is default (Saver omits it).
pub fn set_ref_is_default(obj: &ObjectRef, is_default: bool) {
    REF_IS_DEFAULT.with(|m| m.borrow_mut().insert(object_key(obj), is_default));
}

/// The recorded `isDefault` flag for `obj`, if any.
pub fn ref_is_default(obj: &ObjectRef) -> Option<bool> {
    REF_IS_DEFAULT.with(|m| m.borrow().get(&object_key(obj)).copied())
}

/// Record one unmapped XML element of `obj` for verbatim replay on save (the
/// C++ `AutosarResource::addUnknownContent` under
/// `OPTION_RECORD_UNKNOWN_FEATURE`).
pub fn push_unknown_content(obj: &ObjectRef, el: dom::Element) {
    UNKNOWN_CONTENT.with(|m| m.borrow_mut().entry(object_key(obj)).or_default().push(el));
}

/// The unmapped XML elements recorded for `obj`, if any.
pub fn unknown_content(obj: &ObjectRef) -> Option<Vec<dom::Element>> {
    UNKNOWN_CONTENT.with(|m| m.borrow().get(&object_key(obj)).cloned())
}

/// Record the original `DEST` of the reference from `owner.feature` to `target`.
pub fn set_ref_dest(owner: &ObjectRef, feature: &str, target: &ObjectRef, dest: impl Into<String>) {
    let key = (object_key(owner), feature.to_string(), object_key(target));
    REF_DEST.with(|m| m.borrow_mut().insert(key, dest.into()));
}

/// The original `DEST` of the reference from `owner.feature` to `target`, if any.
pub fn ref_dest(owner: &ObjectRef, feature: &str, target: &ObjectRef) -> Option<String> {
    let key = (object_key(owner), feature.to_string(), object_key(target));
    REF_DEST.with(|m| m.borrow().get(&key).cloned())
}

/// Move the original `DEST` record from `old_target` to `new_target` (used when
/// a proxy's pending reference is resolved in place).
pub fn move_ref_dest(
    owner: &ObjectRef,
    feature: &str,
    old_target: &ObjectRef,
    new_target: &ObjectRef,
) {
    let old_key = (
        object_key(owner),
        feature.to_string(),
        object_key(old_target),
    );
    let new_key = (
        object_key(owner),
        feature.to_string(),
        object_key(new_target),
    );
    REF_DEST.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(v) = m.remove(&old_key) {
            m.insert(new_key, v);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::value::Val;
    use emf_ecore::{DynamicEObject, EClass, EClassKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn obj() -> ObjectRef {
        Rc::new(RefCell::new(DynamicEObject::new(EClass::new(
            "X",
            EClassKind::Class,
        ))))
    }

    #[test]
    fn mixed_content_round_trips_in_order() {
        let a = obj();
        let b = obj();
        push_mixed_content(&a, MixedEntry::Text(" ".into()));
        push_mixed_content(&a, MixedEntry::Element(b.clone()));
        let seq = mixed_content(&a).unwrap();
        assert_eq!(seq.len(), 2);
        assert!(matches!(&seq[0], MixedEntry::Text(t) if t == " "));
    }

    #[test]
    fn ref_dest_is_keyed_by_owner_feature_target() {
        let owner = obj();
        let target = obj();
        set_ref_dest(&owner, "PACKAGE-REF", &target, "AR-PACKAGE");
        assert_eq!(
            ref_dest(&owner, "PACKAGE-REF", &target).as_deref(),
            Some("AR-PACKAGE")
        );
        let other = obj();
        move_ref_dest(&owner, "PACKAGE-REF", &target, &other);
        assert_eq!(ref_dest(&owner, "PACKAGE-REF", &target), None);
        assert_eq!(
            ref_dest(&owner, "PACKAGE-REF", &other).as_deref(),
            Some("AR-PACKAGE")
        );
    }

    #[test]
    fn identity_key_distinguishes_objects() {
        let a = obj();
        let b = obj();
        assert_ne!(object_key(&a), object_key(&b));
        set_ref_is_default(&a, true);
        assert_eq!(ref_is_default(&a), Some(true));
        assert_eq!(ref_is_default(&b), None);
        let _ = Val::Null;
    }
}
