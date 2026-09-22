//! C++ parity suite: emf-ecore EOperation metadata.
//!
//! Ports `EObjectEInvokeTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/`.
//!
//! The `eInvoke` + `EInvocationDelegate` dispatch machinery has no Rust
//! counterpart yet (tracked in PARITY_TRACKER as a gap); the EOperation
//! metadata group below ports 1:1.
use emf_ecore::{EClass, EClassKind, EOperation};

struct OpModel {
    cls: EClass,
    add_op: EOperation,
    greet_op: EOperation,
    void_op: EOperation,
}

fn make_op_model() -> OpModel {
    let mut cls = EClass::new("Calculator", EClassKind::Class);
    let mut add = EOperation::new("add");
    add.set_operation_id(0);
    let mut greet = EOperation::new("greet");
    greet.set_operation_id(1);
    let mut reset = EOperation::new("reset");
    reset.set_operation_id(2);
    cls.add_operation(add.clone());
    cls.add_operation(greet.clone());
    cls.add_operation(reset.clone());
    OpModel {
        cls,
        add_op: add,
        greet_op: greet,
        void_op: reset,
    }
}

fn get_op<'a>(cls: &'a EClass, name: &str) -> Option<&'a EOperation> {
    cls.e_operations().iter().find(|o| o.name() == name)
}

#[test]
fn operation_id_of_cloned_ops() {
    // `add_operation` stores the same values; build from the class ops.
    let m = make_op_model();
    assert_eq!(get_op(&m.cls, "add").unwrap().operation_id(), 0);
    assert_eq!(get_op(&m.cls, "greet").unwrap().operation_id(), 1);
    assert_eq!(get_op(&m.cls, "reset").unwrap().operation_id(), 2);
}

#[test]
fn get_operation_by_name() {
    let m = make_op_model();
    assert_eq!(get_op(&m.cls, "add").unwrap().name(), "add");
    assert_eq!(get_op(&m.cls, "greet").unwrap().name(), "greet");
    assert_eq!(get_op(&m.cls, "reset").unwrap().name(), "reset");
    assert!(get_op(&m.cls, "nonexistent").is_none());
}

#[test]
fn operation_count_contains_all() {
    let m = make_op_model();
    assert_eq!(m.cls.e_operations().len(), 3);
}

#[test]
fn operation_id_set_round_trip() {
    let m = make_op_model();
    let mut add = m.add_op.clone();
    assert_eq!(add.operation_id(), 0);
    add.set_operation_id(99);
    assert_eq!(add.operation_id(), 99);
}
