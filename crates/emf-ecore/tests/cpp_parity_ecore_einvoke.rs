//! C++ parity suite: emf-ecore EOperation metadata + eInvoke dispatch.
//!
//! Ports `EObjectEInvokeTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/`.
//!
//! The C++ harness subclasses `BasicEObject` with a delegate map keyed by
//! `EOperation*`; here `InvokableObject` keys the same map by
//! `EOperation.operationID` (Rust has no feature/operation pointer identity at
//! this layer) and passes `None` where C++ passes a null `EObject*`. The
//! delegate contract (`dynamic_invoke`, empty result = void) is 1:1.
use emf_common::eobject::{EObject, InvokeError};
use emf_common::value::Val;
use emf_ecore::invocation::EInvocationDelegate;
use emf_ecore::{EClass, EClassKind, EOperation};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

struct OpModel {
    cls: EClass,
    add_op: EOperation,
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
    cls.add_operation(greet);
    cls.add_operation(reset);
    OpModel { cls, add_op: add }
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

// ===== invocation harness =====

/// Address of the receiver's data, for identity checks (C++ `== &obj`).
fn data_ptr(obj: &InvokableObject) -> *const () {
    obj as *const InvokableObject as *const ()
}

/// An `EObject` that dispatches `e_invoke` to registered delegates.
struct InvokableObject {
    e_class: String,
    delegates: HashMap<i32, Rc<dyn EInvocationDelegate>>,
}

impl InvokableObject {
    fn new(e_class: impl Into<String>) -> Self {
        Self {
            e_class: e_class.into(),
            delegates: HashMap::new(),
        }
    }
    fn set_e_invocation_delegate(
        &mut self,
        operation_id: i32,
        delegate: Rc<dyn EInvocationDelegate>,
    ) {
        self.delegates.insert(operation_id, delegate);
    }
}

impl std::fmt::Debug for InvokableObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvokableObject")
            .field("e_class", &self.e_class)
            .field("delegates", &self.delegates.len())
            .finish()
    }
}

impl EObject for InvokableObject {
    fn e_class(&self) -> &str {
        &self.e_class
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn e_invoke(&self, operation_id: i32, arguments: &[Val]) -> Result<Option<Val>, InvokeError> {
        match self.delegates.get(&operation_id) {
            Some(d) => Ok(d.dynamic_invoke(Some(self), arguments)),
            None => Err(InvokeError),
        }
    }
}

/// Ports C++ `AddDelegate`: sums the first two integer arguments and records
/// the receiver + argument count.
struct AddDelegate {
    last_target: RefCell<Option<*const ()>>,
    last_arg_count: Cell<usize>,
}

impl AddDelegate {
    fn new() -> Self {
        Self {
            last_target: RefCell::new(None),
            last_arg_count: Cell::new(0),
        }
    }
    fn last_target(&self) -> Option<*const ()> {
        *self.last_target.borrow()
    }
    fn last_arg_count(&self) -> usize {
        self.last_arg_count.get()
    }
}

impl EInvocationDelegate for AddDelegate {
    fn dynamic_invoke(&self, target: Option<&dyn EObject>, arguments: &[Val]) -> Option<Val> {
        *self.last_target.borrow_mut() = target.map(|t| t as *const dyn EObject as *const ());
        self.last_arg_count.set(arguments.len());
        let a = arguments.first().and_then(|v| v.as_int()).unwrap_or(0);
        let b = arguments.get(1).and_then(|v| v.as_int()).unwrap_or(0);
        Some(Val::Int(a + b))
    }
}

/// Ports C++ `GreetDelegate`: returns `"hello"`.
struct GreetDelegate;

impl EInvocationDelegate for GreetDelegate {
    fn dynamic_invoke(&self, _target: Option<&dyn EObject>, _arguments: &[Val]) -> Option<Val> {
        Some(Val::String("hello".into()))
    }
}

/// Ports C++ `VoidDelegate`: no return value, records being called.
struct VoidDelegate {
    called: Cell<bool>,
}

impl VoidDelegate {
    fn new() -> Self {
        Self {
            called: Cell::new(false),
        }
    }
    fn was_called(&self) -> bool {
        self.called.get()
    }
}

impl EInvocationDelegate for VoidDelegate {
    fn dynamic_invoke(&self, _target: Option<&dyn EObject>, _arguments: &[Val]) -> Option<Val> {
        self.called.set(true);
        None
    }
}

/// Ports C++ `RecordingDelegate`: records the receiver and arguments.
#[derive(Default)]
struct RecordingDelegate {
    target: RefCell<Option<*const ()>>,
    args: RefCell<Vec<Val>>,
}

impl RecordingDelegate {
    fn target(&self) -> Option<*const ()> {
        *self.target.borrow()
    }
    fn args(&self) -> Vec<Val> {
        self.args.borrow().clone()
    }
}

impl EInvocationDelegate for RecordingDelegate {
    fn dynamic_invoke(&self, target: Option<&dyn EObject>, arguments: &[Val]) -> Option<Val> {
        *self.target.borrow_mut() = target.map(|t| t as *const dyn EObject as *const ());
        *self.args.borrow_mut() = arguments.to_vec();
        None
    }
}

// ===== default eInvoke =====

#[test]
fn invoke_default_errors() {
    // Ports EInvoke_Default_Throws: no delegate -> base contract errors.
    let obj = InvokableObject::new("Calculator");
    assert_eq!(
        obj.e_invoke(0, &[Val::Int(1), Val::Int(2)]),
        Err(InvokeError)
    );
}

#[test]
fn invoke_null_operation_errors() {
    // Ports EInvoke_NullOperation_Throws: C++ passes null -> throws; here an
    // unregistered operation id (-1) stands in for "no operation".
    let obj = InvokableObject::new("Calculator");
    assert_eq!(obj.e_invoke(-1, &[]), Err(InvokeError));
}

// ===== delegate dispatch =====

#[test]
fn invoke_delegates_to_add_delegate() {
    // Ports EInvoke_DelegatesToAddDelegate.
    let mut obj = InvokableObject::new("Calculator");
    obj.set_e_invocation_delegate(0, Rc::new(AddDelegate::new()));
    assert_eq!(
        obj.e_invoke(0, &[Val::Int(3), Val::Int(4)]),
        Ok(Some(Val::Int(7)))
    );
}

#[test]
fn invoke_delegate_receives_target_and_args() {
    // Ports EInvoke_DelegateReceivesTargetAndArgs.
    let mut obj = InvokableObject::new("Calculator");
    let rec = Rc::new(RecordingDelegate::default());
    obj.set_e_invocation_delegate(0, rec.clone());
    obj.e_invoke(0, &[Val::Int(10), Val::Int(20), Val::Int(30)])
        .unwrap();
    assert_eq!(rec.target(), Some(data_ptr(&obj)));
    assert_eq!(rec.args().len(), 3);
    assert_eq!(rec.args()[0], Val::Int(10));
    assert_eq!(rec.args()[1], Val::Int(20));
    assert_eq!(rec.args()[2], Val::Int(30));
}

#[test]
fn invoke_delegate_receives_empty_args() {
    // Ports EInvoke_DelegateReceivesEmptyArgs.
    let mut obj = InvokableObject::new("Calculator");
    let rec = Rc::new(RecordingDelegate::default());
    obj.set_e_invocation_delegate(1, rec.clone());
    obj.e_invoke(1, &[]).unwrap();
    assert_eq!(rec.args().len(), 0);
    assert_eq!(rec.target(), Some(data_ptr(&obj)));
}

#[test]
fn invoke_greet_delegate_returns_string() {
    // Ports EInvoke_GreetDelegate_ReturnsString.
    let mut obj = InvokableObject::new("Calculator");
    obj.set_e_invocation_delegate(1, Rc::new(GreetDelegate));
    assert_eq!(obj.e_invoke(1, &[]), Ok(Some(Val::String("hello".into()))));
}

#[test]
fn invoke_void_delegate_returns_none() {
    // Ports EInvoke_VoidDelegate_ReturnsEmptyAny: empty result = void.
    let mut obj = InvokableObject::new("Calculator");
    let d = Rc::new(VoidDelegate::new());
    obj.set_e_invocation_delegate(2, d.clone());
    assert_eq!(obj.e_invoke(2, &[]), Ok(None));
    assert!(d.was_called());
}

#[test]
fn invoke_multiple_operations_distinct_delegates() {
    // Ports EInvoke_MultipleOperations_DistinctDelegates.
    let mut obj = InvokableObject::new("Calculator");
    obj.set_e_invocation_delegate(0, Rc::new(AddDelegate::new()));
    obj.set_e_invocation_delegate(1, Rc::new(GreetDelegate));
    obj.set_e_invocation_delegate(2, Rc::new(VoidDelegate::new()));
    assert_eq!(
        obj.e_invoke(0, &[Val::Int(5), Val::Int(6)]),
        Ok(Some(Val::Int(11)))
    );
    assert_eq!(obj.e_invoke(1, &[]), Ok(Some(Val::String("hello".into()))));
    assert_eq!(obj.e_invoke(2, &[]), Ok(None));
}

#[test]
fn invoke_unregistered_operation_errors() {
    // Ports EInvoke_UnregisteredOperation_Throws.
    let mut obj = InvokableObject::new("Calculator");
    obj.set_e_invocation_delegate(0, Rc::new(AddDelegate::new()));
    assert_eq!(obj.e_invoke(1, &[]), Err(InvokeError));
}

// ===== EInvocationDelegate direct =====

#[test]
fn invocation_delegate_dynamic_invoke_direct() {
    // Ports EInvocationDelegate_DynamicInvoke_Direct.
    let obj = InvokableObject::new("Calculator");
    let d = AddDelegate::new();
    let result = d.dynamic_invoke(Some(&obj), &[Val::Int(100), Val::Int(200)]);
    assert_eq!(result, Some(Val::Int(300)));
    assert_eq!(d.last_target(), Some(data_ptr(&obj)));
    assert_eq!(d.last_arg_count(), 2);
}

#[test]
fn invocation_delegate_dynamic_invoke_single_arg() {
    // Ports EInvocationDelegate_DynamicInvoke_SingleArg (target null, one arg).
    let d = AddDelegate::new();
    assert_eq!(d.dynamic_invoke(None, &[Val::Int(42)]), Some(Val::Int(42)));
}

#[test]
fn invocation_delegate_dynamic_invoke_no_args() {
    // Ports EInvocationDelegate_DynamicInvoke_NoArgs (target null, no args).
    let d = AddDelegate::new();
    assert_eq!(d.dynamic_invoke(None, &[]), Some(Val::Int(0)));
}

// ===== eDerivedOperationID =====

#[test]
fn derived_operation_id_default_minus_one() {
    // Ports EDerivedOperationID_DefaultReturnsMinusOne.
    let obj = InvokableObject::new("Calculator");
    assert_eq!(obj.e_derived_operation_id(0), -1);
    assert_eq!(obj.e_derived_operation_id(-1), -1);
}

// ===== replace a registered delegate =====

/// Ports the local `FixedDelegate`: always returns 999.
struct FixedDelegate;

impl EInvocationDelegate for FixedDelegate {
    fn dynamic_invoke(&self, _target: Option<&dyn EObject>, _arguments: &[Val]) -> Option<Val> {
        Some(Val::Int(999))
    }
}

#[test]
fn invoke_replace_delegate() {
    // Ports EInvoke_ReplaceDelegate.
    let mut obj = InvokableObject::new("Calculator");
    obj.set_e_invocation_delegate(0, Rc::new(AddDelegate::new()));
    assert_eq!(
        obj.e_invoke(0, &[Val::Int(1), Val::Int(2)]),
        Ok(Some(Val::Int(3)))
    );
    obj.set_e_invocation_delegate(0, Rc::new(FixedDelegate));
    assert_eq!(
        obj.e_invoke(0, &[Val::Int(1), Val::Int(2)]),
        Ok(Some(Val::Int(999)))
    );
}
