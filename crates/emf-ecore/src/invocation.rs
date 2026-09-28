//! Operation invocation delegates (C++ `emf-ecore/EInvocationDelegate`,
//! aligned to EMF `EOperation.Internal.InvocationDelegate`).
//!
//! A delegate is the behavior behind an `EOperation`: it executes the operation
//! on a target `EObject` and returns a value. As in C++, a `None` result means
//! *void* (no return value), and the delegate never panics for control flow —
//! [`EObject::e_invoke`](emf_common::eobject::EObject::e_invoke) reports the
//! "no delegate" case via its `Err` variant instead.

use emf_common::eobject::EObject;
use emf_common::value::Val;

/// The behavior of one operation (C++ `EInvocationDelegate`).
pub trait EInvocationDelegate {
    /// Execute this operation on `target` with `arguments` (aligned 1:1 with
    /// the operation's parameters). `None` models a void return. `target` is
    /// `None` when there is no receiver (C++ passes a null `EObject*`).
    fn dynamic_invoke(&self, target: Option<&dyn EObject>, arguments: &[Val]) -> Option<Val>;
}
