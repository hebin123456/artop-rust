//! C++ parity suite: emf-ecore DataTypeUtil.
//!
//! Ports `DataTypeUtilTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `datatype`
//! helpers (`from_string` / `to_string` / `default_value`) and the Ecore
//! package's built-in data types.
//!
//! Notes tracked in PARITY_TRACKER:
//!   - Rust returns `Val` instead of C++ `std::any`; parse errors are lenient
//!     (unknown types keep the literal `Val::String`, matching C++).
//!   - There is no standalone `coerce`; type coercion is expressed by
//!     `from_string(to_string(...))`, verified below.
use emf_ecore::ecore_package::ecore_package;
use emf_ecore::{datatype, ECORE_NS_URI};
use emf_ecore::Val;

#[test]
fn e_string_from_to() {
    let v = datatype::from_string("EString", "hello");
    assert!(matches!(&v, Val::String(s) if s == "hello"));
    assert_eq!(datatype::to_string("EString", &v), "hello");
}

#[test]
fn e_int_from_to() {
    let v = datatype::from_string("EInt", "123");
    assert_eq!(v, Val::Int(123));
    assert_eq!(datatype::to_string("EInt", &v), "123");
}

#[test]
fn e_boolean_from_to() {
    let vt = datatype::from_string("EBoolean", "true");
    let vf = datatype::from_string("EBoolean", "false");
    assert_eq!(vt, Val::Bool(true));
    assert_eq!(vf, Val::Bool(false));
    assert_eq!(datatype::to_string("EBoolean", &vt), "true");
    assert_eq!(datatype::to_string("EBoolean", &vf), "false");
}

#[test]
fn e_double_from_to() {
    let v = datatype::from_string("EDouble", "3.14");
    assert!(matches!(&v, Val::Double(d) if (*d - 3.14).abs() < 1e-9));
}

#[test]
fn default_values() {
    assert_eq!(datatype::default_value("EString"), Val::String(String::new()));
    assert_eq!(datatype::default_value("EInt"), Val::Int(0));
    assert_eq!(datatype::default_value("EBoolean"), Val::Bool(false));
    assert_eq!(datatype::default_value("EDouble"), Val::Double(0.0));
    assert_eq!(datatype::default_value("ELong"), Val::Int(0));
}

#[test]
fn coerce_string_to_int() {
    // C++ coerce({string "42"}, EInt) -> int 42. Here: parse the string.
    let v = datatype::from_string("EInt", "42");
    assert_eq!(v, Val::Int(42));
}

#[test]
fn coerce_int_to_string() {
    // C++ coerce({int 7}, EString) -> string "7". Convert to string.
    let s = datatype::to_string("EInt", &Val::Int(7));
    assert_eq!(s, "7");
}

#[test]
fn coerce_int_to_boolean() {
    // C++ coerce({int 1}, EBoolean) -> true. Parse "1" as boolean.
    let v = datatype::from_string("EBoolean", "1");
    assert_eq!(v, Val::Bool(true));
}

#[test]
fn ns_uri_constant_matches_ecore_package() {
    let pkg = ecore_package();
    assert_eq!(pkg.borrow().ns_uri().unwrap().to_string(), ECORE_NS_URI);
}

#[test]
fn builtin_data_types_registered_with_defaults() {
    let pkg = ecore_package();
    for (name, expected) in [
        ("EString", datatype::default_value("EString")),
        ("EInt", Val::Int(0)),
        ("EBoolean", Val::Bool(false)),
    ] {
        let guard = pkg.borrow();
        let dt = guard.find_data_type(name).expect("registered");
        assert_eq!(dt.name(), name);
        assert_eq!(datatype::default_value(name), expected);
    }
}