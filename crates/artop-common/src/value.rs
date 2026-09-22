//! Runtime value envelope, the type-safe replacement for C++ `std::any`
//! (and Java's `Object`). A structural feature's value is stored/delivered as a
//! [`Val`], which can be an atomic, an array, or an object reference.
//!
//! Port target: C++ `std::any` usage throughout `emf-common` / `emf-ecore`.

use crate::eobject::EObject;
use std::cell::RefCell;
use std::rc::Rc;

/// An object handle: a shared, mutable reference to an `EObject`.
pub type ObjectRef = Rc<RefCell<dyn EObject>>;

/// Runtime value. Models an `EStructuralFeature` value.
#[derive(Debug, Clone)]
pub enum Val {
    /// An integer value (EMF `EInt` / `EIntegerObject` / `ELong`).
    Int(i64),
    /// A floating point value (EMF `EDouble` / `EFloat`).
    Double(f64),
    /// A string value (EMF `EString`).
    String(String),
    /// A boolean value (EMF `EBoolean` / `EBooleanObject`).
    Bool(bool),
    /// A byte / short (EMF `EByte` / `EShort`).
    Byte(u8),
    /// An enum literal name (EMF `EEnumLiteral.name`).
    EnumLiteral(String),
    /// A single object reference (EMF reference feature, lowerBound<=1).
    Object(ObjectRef),
    /// A list of objects / atomic values (EMF multi-valued feature).
    List(Vec<Val>),
    /// An unset / null value.
    Null,
}

impl PartialEq for Val {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Val::Int(a), Val::Int(b)) => a == b,
            (Val::Double(a), Val::Double(b)) => a == b,
            (Val::String(a), Val::String(b)) => a == b,
            (Val::Bool(a), Val::Bool(b)) => a == b,
            (Val::Byte(a), Val::Byte(b)) => a == b,
            (Val::EnumLiteral(a), Val::EnumLiteral(b)) => a == b,
            (Val::Object(a), Val::Object(b)) => Rc::ptr_eq(a, b),
            (Val::List(a), Val::List(b)) => a == b,
            (Val::Null, Val::Null) => true,
            _ => false,
        }
    }
}

impl Val {
    /// Build a string value.
    pub fn string(s: impl Into<String>) -> Self {
        Val::String(s.into())
    }

    /// Build an object value.
    pub fn object(o: ObjectRef) -> Self {
        Val::Object(o)
    }

    /// Read an integer out of an integer-valued `Val`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Val::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Read a double out of a double-valued `Val`.
    pub fn as_double(&self) -> Option<f64> {
        match self {
            Val::Double(d) => Some(*d),
            _ => None,
        }
    }

    /// Read a string out of a string-valued `Val`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::String(s) => Some(s.as_str()),
            Val::EnumLiteral(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Read a boolean out of a boolean-valued `Val`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Val::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Read an object out of an object-valued `Val`.
    pub fn as_object(&self) -> Option<&ObjectRef> {
        match self {
            Val::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Read the list out of a list-valued `Val`.
    pub fn as_list(&self) -> Option<&[Val]> {
        match self {
            Val::List(l) => Some(l),
            _ => None,
        }
    }

    /// Whether this value is `Null` (unset).
    pub fn is_null(&self) -> bool {
        matches!(self, Val::Null)
    }

    /// Human-readable description used by diagnostics.
    pub fn describe(&self) -> String {
        match self {
            Val::Int(v) => format!("{v}"),
            Val::Double(v) => format!("{v}"),
            Val::String(s) => format!("\"{s}\""),
            Val::Bool(b) => format!("{b}"),
            Val::Byte(b) => format!("{b}"),
            Val::EnumLiteral(e) => format!("{e}"),
            Val::Object(o) => {
                let klass = o.borrow().e_class().to_string();
                format!("<{klass}@{:p}>", o.as_ptr())
            }
            Val::List(l) => format!("[{}]", l.len()),
            Val::Null => "null".into(),
        }
    }
}

impl From<i64> for Val {
    fn from(v: i64) -> Self {
        Val::Int(v)
    }
}
impl From<i32> for Val {
    fn from(v: i32) -> Self {
        Val::Int(v as i64)
    }
}
impl From<f64> for Val {
    fn from(v: f64) -> Self {
        Val::Double(v)
    }
}
impl From<String> for Val {
    fn from(v: String) -> Self {
        Val::String(v)
    }
}
impl From<&str> for Val {
    fn from(v: &str) -> Self {
        Val::String(v.to_string())
    }
}
impl From<bool> for Val {
    fn from(v: bool) -> Self {
        Val::Bool(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_conversions() {
        assert_eq!(Val::Int(5).as_int(), Some(5));
        assert_eq!(Val::Double(2.5).as_double(), Some(2.5));
        assert_eq!(Val::string("hi").as_str(), Some("hi"));
        assert_eq!(Val::Bool(true).as_bool(), Some(true));
    }

    #[test]
    fn list_and_null() {
        assert!(Val::Null.is_null());
        let list = Val::List(vec![Val::Int(1), Val::Int(2)]);
        assert_eq!(list.as_list().map(|l| l.len()), Some(2));
        assert!(Val::Int(3).as_list().is_none());
    }
}
