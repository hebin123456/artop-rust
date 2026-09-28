//! `EditUtil` — Edit-framework static helpers (port of C++ `emf-edit`
//! `util/EditUtil`, aligned to Java `org.eclipse.emf.edit.util.EditUtil`).
//!
//! The C++ unit is a skeleton: `getText`/`getString` are placeholders that
//! delegate to a not-yet-ported label provider, and the container helpers
//! return constant results. This port mirrors that surface so downstream code
//! can compile against the same API; the behaviour-identical tests land later
//! together with the label provider.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use emf_common::value::{ObjectRef, Val};

/// Label text for `value` (C++ `EditUtil::getText`). With no label provider
/// ported yet, an empty `std::any` yields the empty string and any present
/// value yields the C++ placeholder.
pub fn get_text(value: &Val) -> String {
    if value.is_null() {
        String::new()
    } else {
        "<not implemented>".to_string()
    }
}

/// Alias of [`get_text`] (C++ `EditUtil::getString`).
pub fn get_string(value: &Val) -> String {
    get_text(value)
}

/// Find an element named `name` among `elements` (C++ `findElementByName`).
/// The C++ unit is a `nullptr` TODO; this port mirrors it with `None`.
pub fn find_element_by_name(elements: &[Val], name: &str) -> Option<Val> {
    let _ = (elements, name);
    None
}

/// Whether `object` is editable (C++ `isEditable`; skeleton returns `false`).
pub fn is_editable(object: &ObjectRef) -> bool {
    let _ = object;
    false
}

/// Whether `object` is read-only (C++ `isReadOnly`; skeleton returns `false`).
pub fn is_read_only(object: &ObjectRef) -> bool {
    let _ = object;
    false
}

/// Whether `object` is set (C++ `isSet`; skeleton returns `false`).
pub fn is_set(object: &ObjectRef) -> bool {
    let _ = object;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_of_empty_value_is_empty() {
        assert_eq!(get_text(&Val::Null), "");
        assert_eq!(get_string(&Val::Null), "");
    }

    #[test]
    fn text_of_present_value_is_placeholder() {
        assert_eq!(get_text(&Val::string("x")), "<not implemented>");
    }
}
