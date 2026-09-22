//! C++ parity suite: emf-common Command framework.
//!
//! Ports `CommandTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-common/tests/`.
use emf_common::command::{
    AbortExecutionException, AbstractBase, BasicCommandStack, Command, CommandRef,
    CommandStackListener, CommandWrapper, CompoundCommand, IdentityCommand, StrictCompoundCommand,
    UnexecutableCommand, ABSTRACT_DEFAULT_DESCRIPTION, ABSTRACT_DEFAULT_LABEL,
    IDENTITY_DEFAULT_DESCRIPTION, IDENTITY_DEFAULT_LABEL, UNEXECUTABLE_DEFAULT_LABEL,
};
use emf_common::value::Val;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

#[derive(Clone, Copy, Default)]
struct Flags {
    executed: bool,
    undone: bool,
    redone: bool,
}

struct TestCommand {
    common: AbstractBase,
    flags: Rc<RefCell<Flags>>,
    prepare_ok: bool,
}

impl TestCommand {
    fn make(label: &str) -> (CommandRef, Rc<RefCell<Flags>>) {
        Self::make_with(label, true)
    }
    fn make_with(label: &str, prepare_ok: bool) -> (CommandRef, Rc<RefCell<Flags>>) {
        let flags = Rc::new(RefCell::new(Flags::default()));
        let tc = Rc::new(RefCell::new(TestCommand {
            common: AbstractBase {
                label: RefCell::new(label.to_string()),
                ..Default::default()
            },
            flags: flags.clone(),
            prepare_ok,
        }));
        let cr: CommandRef = tc.clone() as Rc<RefCell<dyn Command>>;
        *tc.borrow().common.self_ref.borrow_mut() = Some(cr.clone());
        (cr, flags)
    }
}

impl Command for TestCommand {
    fn can_execute(&self) -> bool {
        self.common.can_execute_with(|| self.prepare_ok)
    }
    fn execute(&self) {
        let mut f = self.flags.borrow_mut();
        f.executed = true;
        f.undone = false;
        f.redone = false;
    }
    fn can_undo(&self) -> bool {
        true
    }
    fn undo(&self) {
        let mut f = self.flags.borrow_mut();
        f.executed = false;
        f.undone = true;
        f.redone = false;
    }
    fn redo(&self) {
        let mut f = self.flags.borrow_mut();
        f.executed = true;
        f.undone = false;
        f.redone = true;
    }
    fn get_result(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_label(&self) -> String {
        self.common.default_label(ABSTRACT_DEFAULT_LABEL)
    }
    fn get_description(&self) -> String {
        self.common
            .default_description(ABSTRACT_DEFAULT_DESCRIPTION)
    }
    fn dispose(&self) {}
    fn chain(&self, c: CommandRef) -> CommandRef {
        self.common.chain_to(c)
    }
}

/// A command whose default `undo()` throws (like C++ `AbstractCommand`).
struct ThrowingUndoCmd {
    common: AbstractBase,
}
impl ThrowingUndoCmd {
    fn make(label: &str) -> CommandRef {
        let tc = Rc::new(RefCell::new(ThrowingUndoCmd {
            common: AbstractBase {
                label: RefCell::new(label.to_string()),
                ..Default::default()
            },
        }));
        let cr: CommandRef = tc.clone() as Rc<RefCell<dyn Command>>;
        *tc.borrow().common.self_ref.borrow_mut() = Some(cr.clone());
        cr
    }
}
impl Command for ThrowingUndoCmd {
    fn can_execute(&self) -> bool {
        self.common.can_execute_with(|| false)
    }
    fn execute(&self) {}
    fn can_undo(&self) -> bool {
        true
    }
    fn undo(&self) {
        self.common.default_undo()
    }
    fn redo(&self) {}
    fn get_result(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_label(&self) -> String {
        self.common.default_label(ABSTRACT_DEFAULT_LABEL)
    }
    fn get_description(&self) -> String {
        self.common
            .default_description(ABSTRACT_DEFAULT_DESCRIPTION)
    }
    fn dispose(&self) {}
    fn chain(&self, c: CommandRef) -> CommandRef {
        self.common.chain_to(c)
    }
}

// ---------------------------------------------------------------------------
// AbstractCommand
// ---------------------------------------------------------------------------

#[test]
fn abstract_command_prepare_defaults_to_false() {
    let cmd = ThrowingUndoCmd::make("");
    assert!(!cmd.borrow().can_execute());
}

#[test]
fn abstract_command_can_execute_cached() {
    let (cmd, _) = TestCommand::make("test");
    assert!(cmd.borrow().can_execute());
    assert!(cmd.borrow().can_execute());
}

#[test]
fn abstract_command_get_label_default() {
    let ec = ThrowingUndoCmd::make("");
    assert_eq!(ec.borrow().get_label(), ABSTRACT_DEFAULT_LABEL);
}

#[test]
fn abstract_command_get_description_default() {
    let ec = ThrowingUndoCmd::make("");
    assert_eq!(ec.borrow().get_description(), ABSTRACT_DEFAULT_DESCRIPTION);
}

#[test]
fn abstract_command_undo_throws() {
    let ec = ThrowingUndoCmd::make("");
    let poisoned = catch_unwind(AssertUnwindSafe(|| {
        ec.borrow().undo();
    }));
    assert!(poisoned.is_err());
}

#[test]
fn abstract_command_chain() {
    let (a, _) = TestCommand::make("a");
    let (b, _) = TestCommand::make("b");
    let chained = a.borrow().chain(b);
    assert!(chained.borrow().can_execute());
    chained.borrow().execute();
}

// ---------------------------------------------------------------------------
// CompoundCommand
// ---------------------------------------------------------------------------

#[test]
fn compound_command_empty_cannot_execute() {
    let cc = CompoundCommand::new();
    assert!(!cc.borrow().can_execute());
    assert!(cc.borrow().is_empty());
}

#[test]
fn compound_command_basic_execution() {
    let (a, af) = TestCommand::make("a");
    let (b, bf) = TestCommand::make("b");
    let cc = CompoundCommand::new();
    cc.borrow().append(a);
    cc.borrow().append(b);
    assert!(!cc.borrow().is_empty());
    assert_eq!(cc.borrow().command_count(), 2);
    assert!(cc.borrow().can_execute());
    cc.borrow().execute();
    assert!(af.borrow().executed);
    assert!(bf.borrow().executed);
    cc.borrow().undo();
    assert!(!af.borrow().executed);
    assert!(!bf.borrow().executed);
    cc.borrow().redo();
    assert!(af.borrow().executed);
    assert!(bf.borrow().executed);
}

#[test]
fn compound_command_label_from_last() {
    let (a, _) = TestCommand::make("labelA");
    let (b, _) = TestCommand::make("labelB");
    let cc = CompoundCommand::new();
    cc.borrow().append(a);
    cc.borrow().append(b);
    assert_eq!(cc.borrow().get_label(), "labelB");
}

#[test]
fn compound_command_append_null_ignored() {
    let cc = CompoundCommand::new();
    // C++ append(nullptr) is a no-op; Rust append takes a non-null CommandRef.
    assert!(cc.borrow().is_empty());
}

#[test]
fn compound_command_append_if_can_execute() {
    let (a, _) = TestCommand::make("a");
    let cc = CompoundCommand::new();
    assert!(cc.borrow().append_if_can_execute(a));
    assert!(!cc.borrow().is_empty());
}

#[test]
fn compound_command_unwrap_empty() {
    let cc = CompoundCommand::new();
    let u = cc.borrow().unwrap();
    assert!(!u.borrow().can_execute()); // UnexecutableCommand
}

#[test]
fn compound_command_unwrap_one() {
    let (a, _) = TestCommand::make("a");
    let cc = CompoundCommand::new();
    cc.borrow().append(a);
    let u = cc.borrow().unwrap();
    assert!(u.borrow().can_execute());
}

// ---------------------------------------------------------------------------
// StrictCompoundCommand
// ---------------------------------------------------------------------------

#[test]
fn strict_compound_empty_cannot_execute() {
    let sc = StrictCompoundCommand::new();
    assert!(!sc.borrow().can_execute());
}

#[test]
fn strict_compound_basic_execute() {
    let (a, af) = TestCommand::make("a");
    let (b, bf) = TestCommand::make("b");
    let sc = StrictCompoundCommand::new();
    sc.borrow().append(a);
    sc.borrow().append(b);
    assert!(sc.borrow().can_execute());
    sc.borrow().execute();
    assert!(af.borrow().executed);
    assert!(bf.borrow().executed);
    // Non-pessimistic strict undo only undoes the last command (the C++ fixture
    // first executes the previous ones during prepare). `a` stays executed.
    sc.borrow().undo();
    assert!(af.borrow().executed); // a not undone
    assert!(!bf.borrow().executed); // b undone
}

#[test]
fn strict_compound_pessimistic() {
    let (a, af) = TestCommand::make("a");
    let (b, bf) = TestCommand::make("b");
    let sc = StrictCompoundCommand::new();
    sc.borrow().set_is_pessimistic(true);
    sc.borrow().append(a);
    sc.borrow().append(b);
    assert!(sc.borrow().can_execute());
    sc.borrow().execute();
    assert!(af.borrow().executed);
    assert!(bf.borrow().executed);
    sc.borrow().undo();
    assert!(!af.borrow().executed);
    assert!(!bf.borrow().executed);
}

#[test]
fn strict_compound_append_and_execute() {
    let sc = StrictCompoundCommand::new();
    let (c, cf) = TestCommand::make("c");
    assert!(sc.borrow().append_and_execute(c));
    assert_eq!(sc.borrow().command_count(), 1);
    assert!(cf.borrow().executed);
}

// ---------------------------------------------------------------------------
// UnexecutableCommand / IdentityCommand
// ---------------------------------------------------------------------------

#[test]
fn unexecutable_command_cannot_execute() {
    let u = UnexecutableCommand::instance();
    assert!(!u.borrow().can_execute());
    assert!(!u.borrow().can_undo());
}

#[test]
fn unexecutable_command_label() {
    let u = UnexecutableCommand::instance();
    assert_eq!(u.borrow().get_label(), UNEXECUTABLE_DEFAULT_LABEL);
}

#[test]
fn identity_command_can_execute() {
    let i = IdentityCommand::instance();
    assert!(i.borrow().can_execute());
    i.borrow().execute();
    i.borrow().undo();
    i.borrow().redo();
}

#[test]
fn identity_command_result_empty() {
    let i = IdentityCommand::instance();
    assert!(i.borrow().get_result().is_empty());
}

#[test]
fn identity_command_result_singleton() {
    let i = IdentityCommand::with_result_singleton(Val::String("hello".into()));
    let res = i.borrow().get_result();
    assert_eq!(res.len(), 1);
    match &res[0] {
        Val::String(s) => assert_eq!(s, "hello"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn identity_command_label_description() {
    let i = IdentityCommand::instance();
    assert_eq!(i.borrow().get_label(), IDENTITY_DEFAULT_LABEL);
    assert_eq!(i.borrow().get_description(), IDENTITY_DEFAULT_DESCRIPTION);
}

// ---------------------------------------------------------------------------
// CommandWrapper
// ---------------------------------------------------------------------------

#[test]
fn command_wrapper_delegates_execute() {
    let (inner, f) = TestCommand::make("inner");
    let w = CommandWrapper::new(inner);
    assert!(w.borrow().can_execute());
    w.borrow().execute();
    assert!(f.borrow().executed);
    w.borrow().undo();
    assert!(!f.borrow().executed); // undone implies not executed
    w.borrow().redo();
    assert!(f.borrow().executed);
}

#[test]
fn command_wrapper_can_undo_delegates() {
    let (inner, _) = TestCommand::make("inner");
    let w = CommandWrapper::new(inner);
    assert!(w.borrow().can_undo());
}

#[test]
fn command_wrapper_empty_command() {
    let w = CommandWrapper::empty();
    assert!(!w.borrow().can_execute());
    w.borrow().execute();
    w.borrow().undo();
    w.borrow().redo();
    assert!(w.borrow().get_result().is_empty());
}

#[test]
fn command_wrapper_get_label_delegates() {
    let (inner, _) = TestCommand::make("inner-label");
    let w = CommandWrapper::new(inner);
    assert_eq!(w.borrow().get_label(), "inner-label");
}

#[test]
fn command_wrapper_lazy_create() {
    let created = Rc::new(RefCell::new(false));
    let created_capture = created.clone();
    let stored = Rc::new(RefCell::new(None));
    let stored_capture = stored.clone();
    let w = CommandWrapper::lazy(move || {
        *created_capture.borrow_mut() = true;
        let (c, f) = TestCommand::make("lazy");
        let _ = f;
        *stored_capture.borrow_mut() = Some(c.clone());
        c
    });
    assert!(w.borrow().can_execute());
    assert!(*created.borrow());
    assert!(w.borrow().can_execute());
    w.borrow().execute();
    let inner = stored.borrow().clone().expect("lazy should have created");
    assert!(inner.borrow().can_execute());
}

// ---------------------------------------------------------------------------
// BasicCommandStack
// ---------------------------------------------------------------------------

struct StackListener {
    count: Rc<RefCell<usize>>,
}
impl CommandStackListener for StackListener {
    fn command_stack_changed(&self) {
        *self.count.borrow_mut() += 1;
    }
}

#[test]
fn basic_command_stack_execute_adds_command() {
    let stack = BasicCommandStack::new();
    let (cmd, f) = TestCommand::make("a");
    stack.execute(Some(cmd));
    assert!(stack.get_undo_command().is_some());
    assert!(f.borrow().executed);
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
    stack.flush();
}

#[test]
fn basic_command_stack_undo_redo() {
    let stack = BasicCommandStack::new();
    let (cmd, f) = TestCommand::make("a");
    stack.execute(Some(cmd));
    assert!(stack.can_undo());
    stack.undo();
    assert!(!f.borrow().executed);
    assert!(!stack.can_undo());
    assert!(stack.can_redo());
    stack.redo();
    assert!(stack.can_undo());
    stack.flush();
}

#[test]
fn basic_command_stack_redo_clears_ahead() {
    let stack = BasicCommandStack::new();
    let (a, _) = TestCommand::make("a");
    let (b, _) = TestCommand::make("b");
    stack.execute(Some(a));
    stack.undo();
    stack.execute(Some(b));
    assert!(!stack.can_redo());
    assert!(stack.can_undo());
    stack.flush();
}

#[test]
fn basic_command_stack_most_recent() {
    let stack = BasicCommandStack::new();
    let (a, _) = TestCommand::make("a");
    let (b, _) = TestCommand::make("b");
    stack.execute(Some(a));
    stack.execute(Some(b.clone()));
    let most = stack.get_most_recent_command().expect("some");
    assert!(Rc::ptr_eq(&most, &b));
    stack.flush();
}

#[test]
fn basic_command_stack_flush_clears() {
    let stack = BasicCommandStack::new();
    let (a, _) = TestCommand::make("a");
    stack.execute(Some(a));
    stack.flush();
    assert!(stack.get_undo_command().is_none());
    assert!(stack.get_redo_command().is_none());
    assert!(stack.get_most_recent_command().is_none());
    assert!(!stack.can_undo());
    assert!(!stack.can_redo());
}

#[test]
fn basic_command_stack_listener_notified() {
    let stack = BasicCommandStack::new();
    let count = Rc::new(RefCell::new(0_usize));
    let listener: Rc<dyn CommandStackListener> = Rc::new(StackListener {
        count: count.clone(),
    });
    stack.add_command_stack_listener(listener.clone());
    let (a, _) = TestCommand::make("a");
    stack.execute(Some(a));
    assert_eq!(*count.borrow(), 1);
    stack.undo();
    assert_eq!(*count.borrow(), 2);
    stack.remove_command_stack_listener(&listener);
    let (b, _) = TestCommand::make("b");
    stack.execute(Some(b));
    assert_eq!(*count.borrow(), 2);
    stack.flush();
}

#[test]
fn basic_command_stack_save_index() {
    let stack = BasicCommandStack::new();
    let (a, _) = TestCommand::make("a");
    let (b, _) = TestCommand::make("b");
    stack.execute(Some(a));
    stack.save_is_done();
    assert!(!stack.is_save_needed());
    stack.execute(Some(b));
    assert!(stack.is_save_needed());
    stack.undo();
    assert!(!stack.is_save_needed());
    stack.flush();
}

#[test]
fn basic_command_stack_null_command_ignored() {
    let stack = BasicCommandStack::new();
    stack.execute(None);
    assert!(stack.get_undo_command().is_none());
    assert!(!stack.can_undo());
}

// ---------------------------------------------------------------------------
// AbortExecutionException
// ---------------------------------------------------------------------------

#[test]
fn abort_exception_default_constructor() {
    let ex = AbortExecutionException::new();
    assert_eq!(ex.message(), "");
}

#[test]
fn abort_exception_message_constructor() {
    let ex = AbortExecutionException::with_message("aborted");
    assert_eq!(ex.message(), "aborted");
}

#[test]
fn abort_exception_display() {
    let ex = AbortExecutionException::with_message("abort");
    assert_eq!(ex.to_string(), "abort");
}
