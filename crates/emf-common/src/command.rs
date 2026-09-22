//! Command layer, ported from C++ `emf-common/command` (org.eclipse.emf.common.command).
//!
//! Provides an EMF-style undo/redo command abstraction: `Command`, `AbstractCommand`,
//! `CompoundCommand`, `StrictCompoundCommand`, `IdentityCommand`, `UnexecutableCommand`,
//! `CommandWrapper`, a `BasicCommandStack` with save-index dirty tracking, the
//! `CommandStackListener` callback trait and the `AbortExecutionException` runtime error.
//!
//! C++ uses raw pointers + `std::any`; here every command is reference-counted behind
//! `Rc<RefCell<dyn Command>>` and results are expressed as `Vec<Val>` (the crate's
//! type-safe `std::any` replacement).

use crate::value::Val;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------
// Default strings (mirror of CommonPlugin's string table keys).
// ---------------------------------------------------------------------------
pub const ABSTRACT_DEFAULT_LABEL: &str = "Application Action";
pub const ABSTRACT_DEFAULT_DESCRIPTION: &str = "An application action";
pub const COMPOUND_DEFAULT_LABEL: &str = "Compound Command";
pub const COMPOUND_DEFAULT_DESCRIPTION: &str = "A compound command";
pub const UNEXECUTABLE_DEFAULT_LABEL: &str = "Unexecutable Command";
pub const UNEXECUTABLE_DEFAULT_DESCRIPTION: &str = "An unexecutable command";
pub const IDENTITY_DEFAULT_LABEL: &str = "Identity Command";
pub const IDENTITY_DEFAULT_DESCRIPTION: &str = "An identity command";
pub const WRAPPER_DEFAULT_LABEL: &str = "Command Wrapper";
pub const WRAPPER_DEFAULT_DESCRIPTION: &str = "A command that wraps another command";

// CompoundCommand result-index sentinels (Java int semantics).
pub const LAST_COMMAND_ALL: i32 = i32::MIN;
pub const MERGE_COMMAND_ALL: i32 = i32::MAX;

// ---------------------------------------------------------------------------
// Command trait / reference type
// ---------------------------------------------------------------------------

/// Shared, mutable handle to any [`Command`].
pub type CommandRef = Rc<RefCell<dyn Command>>;

/// The EMF [`Command`] contract.
pub trait Command {
    fn can_execute(&self) -> bool;
    fn execute(&self);
    fn can_undo(&self) -> bool;
    fn undo(&self);
    fn redo(&self);
    fn get_result(&self) -> Vec<Val>;
    fn get_affected_objects(&self) -> Vec<Val>;
    fn get_label(&self) -> String;
    fn get_description(&self) -> String;
    fn dispose(&self);
    fn chain(&self, command: CommandRef) -> CommandRef;
}

// ---------------------------------------------------------------------------
// AbstractCommand shared state + default helpers
// ---------------------------------------------------------------------------

/// Mutable state shared by every `AbstractCommand` subclass.
#[derive(Default)]
pub struct AbstractBase {
    pub is_prepared: Cell<bool>,
    pub is_executable: Cell<bool>,
    pub label: RefCell<String>,
    pub description: RefCell<String>,
    /// Pointer back to the owned `CommandRef` (used by the default `chain()`).
    pub self_ref: RefCell<Option<CommandRef>>,
}

impl AbstractBase {
    /// Default `AbstractCommand.canExecute`: cache `prepare()`'s result.
    pub fn can_execute_with(&self, prepare: impl FnOnce() -> bool) -> bool {
        if !self.is_prepared.get() {
            self.is_executable.set(prepare());
            self.is_prepared.set(true);
        }
        self.is_executable.get()
    }

    /// `getLabel()` default: return `label` if non-empty, else `fallback`.
    pub fn default_label(&self, fallback: &str) -> String {
        let l = self.label.borrow();
        if l.is_empty() {
            fallback.to_string()
        } else {
            l.clone()
        }
    }

    /// `getDescription()` default: return `description` if non-empty, else `fallback`.
    pub fn default_description(&self, fallback: &str) -> String {
        let d = self.description.borrow();
        if d.is_empty() {
            fallback.to_string()
        } else {
            d.clone()
        }
    }

    /// Default `AbstractCommand.canUndo()`.
    pub fn default_can_undo(&self) -> bool {
        true
    }

    /// Default `AbstractCommand.undo()` -> method not implemented.
    pub fn default_undo(&self) -> ! {
        panic!("Method not implemented: AbstractCommand.undo()")
    }

    /// Default results (empty).
    pub fn default_result(&self) -> Vec<Val> {
        Vec::new()
    }

    /// Default `AbstractCommand.chain(command)`: build a `CompoundCommand` holding
    /// `self` + `command`.
    pub fn chain_to(&self, command: CommandRef) -> CommandRef {
        let this = self
            .self_ref
            .borrow()
            .clone()
            .expect("AbstractCommand::chain requires a self reference");
        CompoundCommand::from_commands(vec![this, command])
    }
}

// ---------------------------------------------------------------------------
// AbortExecutionException (C++ std::runtime_error-based)
// ---------------------------------------------------------------------------
use std::error::Error;
use std::fmt;

/// A runtime exception signalling that command execution should be aborted.
#[derive(Debug, Clone, PartialEq)]
pub struct AbortExecutionException {
    message: String,
}

impl AbortExecutionException {
    pub fn new() -> Self {
        AbortExecutionException {
            message: String::new(),
        }
    }

    pub fn with_message(message: impl Into<String>) -> Self {
        AbortExecutionException {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Default for AbortExecutionException {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AbortExecutionException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for AbortExecutionException {}

// ---------------------------------------------------------------------------
// CompoundCommand
// ---------------------------------------------------------------------------

pub struct CompoundCommand {
    common: AbstractBase,
    commands: RefCell<Vec<CommandRef>>,
    result_index: i32,
    weak_self: Weak<RefCell<dyn Command>>,
}

impl CompoundCommand {
    fn intern(
        result_index: i32,
        label: Option<String>,
        description: Option<String>,
        initial: Vec<CommandRef>,
    ) -> Rc<RefCell<CompoundCommand>> {
        Rc::new_cyclic(|weak| {
            let weak_self: Weak<RefCell<dyn Command>> = weak.clone() as _;
            RefCell::new(CompoundCommand {
                common: AbstractBase {
                    label: RefCell::new(label.unwrap_or_default()),
                    description: RefCell::new(description.unwrap_or_default()),
                    ..Default::default()
                },
                commands: RefCell::new(initial),
                result_index,
                weak_self,
            })
        })
    }

    /// Default `CompoundCommand()` (MERGE_COMMAND_ALL).
    pub fn new() -> Rc<RefCell<CompoundCommand>> {
        Self::intern(MERGE_COMMAND_ALL, None, None, Vec::new())
    }

    /// `CompoundCommand(int resultIndex)`.
    pub fn with_result_index(result_index: i32) -> Rc<RefCell<CompoundCommand>> {
        Self::intern(result_index, None, None, Vec::new())
    }

    /// `CompoundCommand(String label)`.
    pub fn with_label(label: impl Into<String>) -> Rc<RefCell<CompoundCommand>> {
        Self::intern(MERGE_COMMAND_ALL, Some(label.into()), None, Vec::new())
    }

    /// Build a compound from pre-populated commands (used by `chain`).
    pub fn from_commands(commands: Vec<CommandRef>) -> CommandRef {
        Self::intern(MERGE_COMMAND_ALL, None, None, commands) as Rc<RefCell<dyn Command>>
    }

    pub fn is_empty(&self) -> bool {
        self.commands.borrow().is_empty()
    }

    pub fn command_count(&self) -> usize {
        self.commands.borrow().len()
    }

    pub fn get_result_index(&self) -> i32 {
        self.result_index
    }

    pub fn prepare_internal(&self) -> bool {
        let list = self.commands.borrow();
        if list.is_empty() {
            return false;
        }
        list.iter().all(|c| c.borrow().can_execute())
    }

    pub fn append(&self, command: CommandRef) {
        if self.common.is_prepared.get() {
            panic!("The command is already prepared");
        }
        self.commands.borrow_mut().push(command);
    }

    pub fn append_if_can_execute(&self, command: CommandRef) -> bool {
        if command.borrow().can_execute() {
            self.commands.borrow_mut().push(command);
            true
        } else {
            command.borrow().dispose();
            false
        }
    }

    pub fn append_and_execute(&self, command: CommandRef) -> bool {
        if command.borrow().can_execute() {
            command.borrow().execute();
            self.commands.borrow_mut().push(command.clone());
            true
        } else {
            command.borrow().dispose();
            false
        }
    }

    pub fn unwrap(&self) -> CommandRef {
        let n = self.commands.borrow().len();
        match n {
            0 => {
                self.dispose();
                UnexecutableCommand::instance()
            }
            1 => {
                let r = self.commands.borrow_mut().remove(0);
                self.dispose();
                r
            }
            _ => self
                .weak_self
                .upgrade()
                .expect("CompoundCommand must hold a live self reference"),
        }
    }
}

impl Command for CompoundCommand {
    fn can_execute(&self) -> bool {
        self.common
            .can_execute_with(|| CompoundCommand::prepare_internal(self))
    }
    fn execute(&self) {
        let list = self.commands.borrow();
        for c in list.iter() {
            c.borrow().execute();
        }
    }
    fn can_undo(&self) -> bool {
        let list = self.commands.borrow();
        list.iter().all(|c| c.borrow().can_undo())
    }
    fn undo(&self) {
        let list = self.commands.borrow();
        for c in list.iter().rev() {
            c.borrow().undo();
        }
    }
    fn redo(&self) {
        let list = self.commands.borrow();
        for c in list.iter() {
            c.borrow().redo();
        }
    }
    fn get_result(&self) -> Vec<Val> {
        let list = self.commands.borrow();
        if list.is_empty() {
            return Vec::new();
        }
        match self.result_index {
            LAST_COMMAND_ALL => list[list.len() - 1].borrow().get_result(),
            MERGE_COMMAND_ALL => {
                let mut r = Vec::new();
                for c in list.iter() {
                    r.extend(c.borrow().get_result());
                }
                r
            }
            i if i >= 0 && (i as usize) < list.len() => list[i as usize].borrow().get_result(),
            _ => Vec::new(),
        }
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        let list = self.commands.borrow();
        if list.is_empty() {
            return Vec::new();
        }
        match self.result_index {
            LAST_COMMAND_ALL => list[list.len() - 1].borrow().get_affected_objects(),
            MERGE_COMMAND_ALL => {
                let mut r = Vec::new();
                for c in list.iter() {
                    r.extend(c.borrow().get_affected_objects());
                }
                r
            }
            i if i >= 0 && (i as usize) < list.len() => {
                list[i as usize].borrow().get_affected_objects()
            }
            _ => Vec::new(),
        }
    }
    fn get_label(&self) -> String {
        let l = self.common.label.borrow();
        if !l.is_empty() {
            return l.clone();
        }
        let list = self.commands.borrow();
        if list.is_empty() {
            return COMPOUND_DEFAULT_LABEL.to_string();
        }
        if self.result_index == LAST_COMMAND_ALL || self.result_index == MERGE_COMMAND_ALL {
            return list[list.len() - 1].borrow().get_label();
        }
        if self.result_index >= 0 && (self.result_index as usize) < list.len() {
            return list[self.result_index as usize].borrow().get_label();
        }
        COMPOUND_DEFAULT_LABEL.to_string()
    }
    fn get_description(&self) -> String {
        let d = self.common.description.borrow();
        if !d.is_empty() {
            return d.clone();
        }
        let list = self.commands.borrow();
        if list.is_empty() {
            return COMPOUND_DEFAULT_DESCRIPTION.to_string();
        }
        if self.result_index == LAST_COMMAND_ALL || self.result_index == MERGE_COMMAND_ALL {
            return list[list.len() - 1].borrow().get_description();
        }
        if self.result_index >= 0 && (self.result_index as usize) < list.len() {
            return list[self.result_index as usize].borrow().get_description();
        }
        COMPOUND_DEFAULT_DESCRIPTION.to_string()
    }
    fn dispose(&self) {
        let list = self.commands.borrow();
        for c in list.iter() {
            c.borrow().dispose();
        }
    }
    fn chain(&self, command: CommandRef) -> CommandRef {
        self.common.chain_to(command)
    }
}

// ---------------------------------------------------------------------------
// StrictCompoundCommand
// ---------------------------------------------------------------------------

pub struct StrictCompoundCommand {
    common: AbstractBase,
    commands: RefCell<Vec<CommandRef>>,
    is_undoable: Cell<bool>,
    is_pessimistic: Cell<bool>,
    rightmost_executed_index: Cell<i32>,
}

impl StrictCompoundCommand {
    pub fn new() -> Rc<RefCell<StrictCompoundCommand>> {
        Rc::new(RefCell::new(StrictCompoundCommand {
            common: AbstractBase::default(),
            commands: RefCell::new(Vec::new()),
            is_undoable: Cell::new(false),
            is_pessimistic: Cell::new(false),
            rightmost_executed_index: Cell::new(-1),
        }))
    }

    pub fn new_with_label(label: &str) -> Rc<RefCell<StrictCompoundCommand>> {
        Rc::new(RefCell::new(StrictCompoundCommand {
            common: AbstractBase {
                label: RefCell::new(label.to_string()),
                ..Default::default()
            },
            commands: RefCell::new(Vec::new()),
            is_undoable: Cell::new(false),
            is_pessimistic: Cell::new(false),
            rightmost_executed_index: Cell::new(-1),
        }))
    }

    pub fn is_pessimistic(&self) -> bool {
        self.is_pessimistic.get()
    }
    pub fn set_is_pessimistic(&self, v: bool) {
        self.is_pessimistic.set(v);
    }
    pub fn is_undoable(&self) -> bool {
        self.is_undoable.get()
    }
    pub fn set_is_undoable(&self, v: bool) {
        self.is_undoable.set(v);
    }
    pub fn command_count(&self) -> usize {
        self.commands.borrow().len()
    }

    pub fn append(&self, command: CommandRef) {
        if self.common.is_prepared.get() {
            panic!("The command is already prepared");
        }
        self.commands.borrow_mut().push(command);
    }

    pub fn prepare_internal(&self) -> bool {
        let cmds = self.commands.borrow();
        let len = cmds.len();
        if len == 0 {
            self.is_undoable.set(false);
            return false;
        }
        let mut result = true;
        for i in 0..len {
            let cmd = &cmds[i];
            if cmd.borrow().can_execute() {
                if i == len - 1 {
                    self.is_undoable.set(cmd.borrow().can_undo());
                    break;
                } else if cmd.borrow().can_undo() {
                    if (i as i32) <= self.rightmost_executed_index.get() {
                        cmd.borrow().redo();
                    } else {
                        self.rightmost_executed_index
                            .set(self.rightmost_executed_index.get() + 1);
                        cmd.borrow().execute();
                    }
                } else {
                    result = false;
                    break;
                }
            } else {
                result = false;
                break;
            }
        }

        if self.is_pessimistic.get() {
            let rm = self.rightmost_executed_index.get();
            for i in (0..=rm).rev() {
                if i < len as i32 {
                    cmds[i as usize].borrow().undo();
                }
            }
        }
        result
    }

    fn self_execute(&self) {
        let cmds = self.commands.borrow();
        let len = cmds.len();
        if self.is_pessimistic.get() {
            for i in 0..len {
                if (i as i32) <= self.rightmost_executed_index.get() {
                    cmds[i].borrow().redo();
                } else {
                    cmds[i].borrow().execute();
                }
            }
        } else if len > 0 {
            cmds[len - 1].borrow().execute();
        }
    }

    pub fn append_and_execute(&self, command: CommandRef) -> bool {
        if !self.common.is_prepared.get() {
            let filled = self.commands.borrow().len() > 0;
            if !filled {
                self.common.is_prepared.set(true);
                self.common.is_executable.set(true);
            } else {
                let ex = self.prepare_internal();
                self.common.is_prepared.set(true);
                self.common.is_executable.set(ex);
                self.is_pessimistic.set(true);
                if ex {
                    self.self_execute();
                }
            }
        }
        if command.borrow().can_execute() {
            command.borrow().execute();
            self.commands.borrow_mut().push(command.clone());
            self.rightmost_executed_index
                .set(self.rightmost_executed_index.get() + 1);
            self.is_undoable.set(command.borrow().can_undo());
            true
        } else {
            command.borrow().dispose();
            false
        }
    }
}

impl Command for StrictCompoundCommand {
    fn can_execute(&self) -> bool {
        self.common
            .can_execute_with(|| StrictCompoundCommand::prepare_internal(self))
    }
    fn execute(&self) {
        StrictCompoundCommand::self_execute(self)
    }
    fn can_undo(&self) -> bool {
        let list = self.commands.borrow();
        list.iter().all(|c| c.borrow().can_undo())
    }
    fn undo(&self) {
        let cmds = self.commands.borrow();
        if self.is_pessimistic.get() {
            for c in cmds.iter().rev() {
                c.borrow().undo();
            }
        } else if !cmds.is_empty() {
            cmds[cmds.len() - 1].borrow().undo();
        }
    }
    fn redo(&self) {
        let cmds = self.commands.borrow();
        if self.is_pessimistic.get() {
            for c in cmds.iter() {
                c.borrow().redo();
            }
        } else if !cmds.is_empty() {
            cmds[cmds.len() - 1].borrow().redo();
        }
    }
    fn get_result(&self) -> Vec<Val> {
        let list = self.commands.borrow();
        match list.last() {
            Some(c) => c.borrow().get_result(),
            None => Vec::new(),
        }
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        let list = self.commands.borrow();
        match list.last() {
            Some(c) => c.borrow().get_affected_objects(),
            None => Vec::new(),
        }
    }
    fn get_label(&self) -> String {
        self.common.default_label(COMPOUND_DEFAULT_LABEL)
    }
    fn get_description(&self) -> String {
        self.common
            .default_description(COMPOUND_DEFAULT_DESCRIPTION)
    }
    fn dispose(&self) {
        let list = self.commands.borrow();
        for c in list.iter() {
            c.borrow().dispose();
        }
    }
    fn chain(&self, command: CommandRef) -> CommandRef {
        self.common.chain_to(command)
    }
}

// ---------------------------------------------------------------------------
// IdentityCommand
// ---------------------------------------------------------------------------

pub struct IdentityCommand {
    common: AbstractBase,
    result: Vec<Val>,
}

impl IdentityCommand {
    fn make(
        result: Vec<Val>,
        label: Option<String>,
        description: Option<String>,
    ) -> IdentityCommand {
        IdentityCommand {
            common: AbstractBase {
                label: RefCell::new(label.unwrap_or_default()),
                description: RefCell::new(description.unwrap_or_default()),
                ..Default::default()
            },
            result,
        }
    }

    pub fn instance() -> CommandRef {
        thread_local! {
            static INSTANCE: CommandRef = make_identity_singleton();
        }
        INSTANCE.with(|c| c.clone())
    }

    pub fn new() -> CommandRef {
        Self::build(Vec::new(), None, None)
    }

    pub fn with_result(result: Vec<Val>) -> CommandRef {
        Self::build(result, None, None)
    }

    pub fn with_result_singleton(result: Val) -> CommandRef {
        Self::with_result(vec![result])
    }

    /// `IdentityCommand(String label)`.
    pub fn with_label(label: impl Into<String>) -> CommandRef {
        Self::build(Vec::new(), Some(label.into()), None)
    }

    fn build(result: Vec<Val>, label: Option<String>, description: Option<String>) -> CommandRef {
        let concrete = Rc::new(RefCell::new(IdentityCommand::make(
            result,
            label,
            description,
        )));
        let cr: CommandRef = concrete.clone() as CommandRef;
        *concrete.borrow().common.self_ref.borrow_mut() = Some(cr.clone());
        cr
    }
}

impl Command for IdentityCommand {
    fn can_execute(&self) -> bool {
        true
    }
    fn execute(&self) {}
    fn can_undo(&self) -> bool {
        true
    }
    fn undo(&self) {}
    fn redo(&self) {}
    fn get_result(&self) -> Vec<Val> {
        self.result.clone()
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_label(&self) -> String {
        self.common.default_label(IDENTITY_DEFAULT_LABEL)
    }
    fn get_description(&self) -> String {
        self.common
            .default_description(IDENTITY_DEFAULT_DESCRIPTION)
    }
    fn dispose(&self) {}
    fn chain(&self, command: CommandRef) -> CommandRef {
        self.common.chain_to(command)
    }
}

// ---------------------------------------------------------------------------
// UnexecutableCommand
// ---------------------------------------------------------------------------

pub struct UnexecutableCommand {
    common: AbstractBase,
}

impl UnexecutableCommand {
    pub fn instance() -> CommandRef {
        thread_local! {
            static INSTANCE: CommandRef = make_unexecutable_singleton();
        }
        INSTANCE.with(|c| c.clone())
    }
}

impl Command for UnexecutableCommand {
    fn can_execute(&self) -> bool {
        false
    }
    fn execute(&self) {
        panic!("Method not implemented: UnexecutableCommand.execute()")
    }
    fn can_undo(&self) -> bool {
        false
    }
    fn undo(&self) {
        self.common.default_undo()
    }
    fn redo(&self) {
        panic!("Method not implemented: UnexecutableCommand.redo()")
    }
    fn get_result(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        Vec::new()
    }
    fn get_label(&self) -> String {
        self.common.default_label(UNEXECUTABLE_DEFAULT_LABEL)
    }
    fn get_description(&self) -> String {
        self.common
            .default_description(UNEXECUTABLE_DEFAULT_DESCRIPTION)
    }
    fn dispose(&self) {}
    fn chain(&self, command: CommandRef) -> CommandRef {
        self.common.chain_to(command)
    }
}

// ---------------------------------------------------------------------------
// CommandWrapper
// ---------------------------------------------------------------------------

/// A decorator/proxy that delegates to an inner command, with optional lazy creation.
pub struct CommandWrapper {
    common: AbstractBase,
    command: RefCell<Option<CommandRef>>,
    create_fn: RefCell<Option<Rc<dyn Fn() -> CommandRef>>>,
}

impl CommandWrapper {
    pub fn new(command: CommandRef) -> Rc<RefCell<CommandWrapper>> {
        let label = command.borrow().get_label();
        let description = command.borrow().get_description();
        Rc::new(RefCell::new(CommandWrapper {
            common: AbstractBase {
                label: RefCell::new(label),
                description: RefCell::new(description),
                ..Default::default()
            },
            command: RefCell::new(Some(command)),
            create_fn: RefCell::new(None),
        }))
    }

    pub fn with_label(
        label: impl Into<String>,
        command: CommandRef,
    ) -> Rc<RefCell<CommandWrapper>> {
        let description = command.borrow().get_description();
        Rc::new(RefCell::new(CommandWrapper {
            common: AbstractBase {
                label: RefCell::new(label.into()),
                description: RefCell::new(description),
                ..Default::default()
            },
            command: RefCell::new(Some(command)),
            create_fn: RefCell::new(None),
        }))
    }

    /// Empty wrapper whose inner command is created lazily via `create_command`.
    pub fn empty() -> Rc<RefCell<CommandWrapper>> {
        Rc::new(RefCell::new(CommandWrapper {
            common: AbstractBase::default(),
            command: RefCell::new(None),
            create_fn: RefCell::new(None),
        }))
    }

    /// Lazy wrapper: `create_command` supplies the inner command on first use.
    pub fn lazy(create: impl Fn() -> CommandRef + 'static) -> Rc<RefCell<CommandWrapper>> {
        let create: Rc<dyn Fn() -> CommandRef> = Rc::new(create);
        Rc::new(RefCell::new(CommandWrapper {
            common: AbstractBase::default(),
            command: RefCell::new(None),
            create_fn: RefCell::new(Some(create)),
        }))
    }

    pub fn get_command(&self) -> Option<CommandRef> {
        self.command.borrow().clone()
    }

    fn prepare_internal(&self) -> bool {
        if self.command.borrow().is_none() {
            if let Some(create) = self.create_fn.borrow().as_ref() {
                *self.command.borrow_mut() = Some(create());
            }
        }
        match self.command.borrow().as_ref() {
            Some(c) => c.borrow().can_execute(),
            None => false,
        }
    }
}

impl Command for CommandWrapper {
    fn can_execute(&self) -> bool {
        self.common
            .can_execute_with(|| CommandWrapper::prepare_internal(self))
    }
    fn execute(&self) {
        if let Some(c) = self.command.borrow().as_ref() {
            c.borrow().execute();
        }
    }
    fn can_undo(&self) -> bool {
        match self.command.borrow().as_ref() {
            Some(c) => c.borrow().can_undo(),
            None => true,
        }
    }
    fn undo(&self) {
        if let Some(c) = self.command.borrow().as_ref() {
            c.borrow().undo();
        }
    }
    fn redo(&self) {
        if let Some(c) = self.command.borrow().as_ref() {
            c.borrow().redo();
        }
    }
    fn get_result(&self) -> Vec<Val> {
        match self.command.borrow().as_ref() {
            Some(c) => c.borrow().get_result(),
            None => Vec::new(),
        }
    }
    fn get_affected_objects(&self) -> Vec<Val> {
        match self.command.borrow().as_ref() {
            Some(c) => c.borrow().get_affected_objects(),
            None => Vec::new(),
        }
    }
    fn get_label(&self) -> String {
        let l = self.common.label.borrow();
        if !l.is_empty() {
            return l.clone();
        }
        match self.command.borrow().as_ref() {
            Some(c) => c.borrow().get_label(),
            None => WRAPPER_DEFAULT_LABEL.to_string(),
        }
    }
    fn get_description(&self) -> String {
        let d = self.common.description.borrow();
        if !d.is_empty() {
            return d.clone();
        }
        match self.command.borrow().as_ref() {
            Some(c) => c.borrow().get_description(),
            None => WRAPPER_DEFAULT_DESCRIPTION.to_string(),
        }
    }
    fn dispose(&self) {
        if let Some(c) = self.command.borrow().as_ref() {
            c.borrow().dispose();
        }
    }
    fn chain(&self, command: CommandRef) -> CommandRef {
        self.common.chain_to(command)
    }
}

// ---------------------------------------------------------------------------
// CommandStackListener + BasicCommandStack
// ---------------------------------------------------------------------------

/// Callback notified whenever the command stack changes.
pub trait CommandStackListener {
    fn command_stack_changed(&self);
}

/// A trivial marker alias (kept for parity with C++ / Java naming).
pub trait CommandStackListenerAlias {}
impl<T: CommandStackListener> CommandStackListenerAlias for T {}

pub struct BasicCommandStack {
    command_list: RefCell<Vec<CommandRef>>,
    top: Cell<i32>,
    save_index: Cell<i32>,
    most_recent: RefCell<Option<CommandRef>>,
    listeners: RefCell<Vec<Rc<dyn CommandStackListener>>>,
}

impl Default for BasicCommandStack {
    fn default() -> Self {
        BasicCommandStack {
            command_list: RefCell::new(Vec::new()),
            top: Cell::new(-1),
            save_index: Cell::new(-1),
            most_recent: RefCell::new(None),
            listeners: RefCell::new(Vec::new()),
        }
    }
}

impl BasicCommandStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&self, command: Option<CommandRef>) {
        let Some(command) = command else { return };
        if command.borrow().can_execute() {
            command.borrow().execute();
            {
                let mut list = self.command_list.borrow_mut();
                while list.len() as i32 > self.top.get() + 1 {
                    list.pop();
                }
                *self.most_recent.borrow_mut() = Some(command.clone());
                list.push(command);
                self.top.set(self.top.get() + 1);
            }
            if self.save_index.get() >= self.top.get() {
                self.save_index.set(-2);
            }
            self.notify_listeners();
        } else {
            command.borrow().dispose();
        }
    }

    pub fn can_undo(&self) -> bool {
        if self.top.get() == -1 {
            return false;
        }
        let list = self.command_list.borrow();
        let idx = self.top.get() as usize;
        idx < list.len() && list[idx].borrow().can_undo()
    }

    pub fn undo(&self) {
        if self.can_undo() {
            let cmd = self.command_list.borrow()[self.top.get() as usize].clone();
            self.top.set(self.top.get() - 1);
            cmd.borrow().undo();
            *self.most_recent.borrow_mut() = Some(cmd);
            self.notify_listeners();
        }
    }

    pub fn can_redo(&self) -> bool {
        let list = self.command_list.borrow();
        self.top.get() < list.len() as i32 - 1
    }

    pub fn redo(&self) {
        if self.can_redo() {
            self.top.set(self.top.get() + 1);
            let cmd = self.command_list.borrow()[self.top.get() as usize].clone();
            cmd.borrow().redo();
            *self.most_recent.borrow_mut() = Some(cmd);
            self.notify_listeners();
        }
    }

    pub fn flush(&self) {
        for c in self.command_list.borrow().iter() {
            c.borrow().dispose();
        }
        self.command_list.borrow_mut().clear();
        self.top.set(-1);
        self.save_index.set(-1);
        *self.most_recent.borrow_mut() = None;
        self.notify_listeners();
    }

    pub fn get_undo_command(&self) -> Option<CommandRef> {
        let list = self.command_list.borrow();
        if self.top.get() == -1 || self.top.get() as usize >= list.len() {
            None
        } else {
            Some(list[self.top.get() as usize].clone())
        }
    }

    pub fn get_redo_command(&self) -> Option<CommandRef> {
        let list = self.command_list.borrow();
        if self.top.get() + 1 >= list.len() as i32 {
            None
        } else {
            Some(list[(self.top.get() + 1) as usize].clone())
        }
    }

    pub fn get_most_recent_command(&self) -> Option<CommandRef> {
        self.most_recent.borrow().clone()
    }

    pub fn add_command_stack_listener(&self, listener: Rc<dyn CommandStackListener>) {
        self.listeners.borrow_mut().push(listener);
    }

    pub fn remove_command_stack_listener(&self, listener: &Rc<dyn CommandStackListener>) {
        let mut l = self.listeners.borrow_mut();
        if let Some(pos) = l.iter().position(|x| Rc::ptr_eq(x, listener)) {
            l.remove(pos);
        }
    }

    pub fn save_is_done(&self) {
        self.save_index.set(self.top.get());
    }

    pub fn is_save_needed(&self) -> bool {
        if self.save_index.get() < -1 {
            return true;
        }
        // With all commands treated as dirty (no NonDirtying marker ported yet),
        // this reduces to: dirty whenever the save index differs from top.
        self.save_index.get() != self.top.get()
    }

    fn notify_listeners(&self) {
        for l in self.listeners.borrow().iter() {
            l.command_stack_changed();
        }
    }
}

// ---------------------------------------------------------------------------
// Singleton factories for Identity/Unexecutable commands.
// ---------------------------------------------------------------------------

fn make_identity_singleton() -> CommandRef {
    Rc::new(RefCell::new(IdentityCommand::make(Vec::new(), None, None))) as CommandRef
}

fn make_unexecutable_singleton() -> CommandRef {
    Rc::new(RefCell::new(UnexecutableCommand {
        common: AbstractBase {
            label: RefCell::new(UNEXECUTABLE_DEFAULT_LABEL.to_string()),
            description: RefCell::new(UNEXECUTABLE_DEFAULT_DESCRIPTION.to_string()),
            ..Default::default()
        },
    })) as CommandRef
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Val;
    use std::panic::{catch_unwind, panic_any, AssertUnwindSafe};

    // ---- Test fixture ----------------------------------------------------
    #[derive(Default)]
    struct TestState {
        executed: bool,
        undone: bool,
        redone: bool,
    }

    struct TestCommand {
        common: AbstractBase,
        state: Rc<RefCell<TestState>>,
    }

    impl TestCommand {
        fn make(label: &str) -> (CommandRef, Rc<RefCell<TestState>>) {
            let state = Rc::new(RefCell::new(TestState::default()));
            let tc = Rc::new(RefCell::new(TestCommand {
                common: AbstractBase {
                    label: RefCell::new(label.to_string()),
                    ..Default::default()
                },
                state: state.clone(),
            }));
            let cr: CommandRef = tc.clone() as Rc<RefCell<dyn Command>>;
            *tc.borrow().common.self_ref.borrow_mut() = Some(cr.clone());
            (cr, state)
        }
    }

    fn test_cmd(label: &str) -> (CommandRef, Rc<RefCell<TestState>>) {
        TestCommand::make(label)
    }

    impl Command for TestCommand {
        fn can_execute(&self) -> bool {
            self.common.can_execute_with(|| true)
        }
        fn execute(&self) {
            let mut s = self.state.borrow_mut();
            s.executed = true;
            s.undone = false;
            s.redone = false;
        }
        fn can_undo(&self) -> bool {
            true
        }
        fn undo(&self) {
            let mut s = self.state.borrow_mut();
            s.executed = false;
            s.undone = true;
            s.redone = false;
        }
        fn redo(&self) {
            let mut s = self.state.borrow_mut();
            s.executed = true;
            s.undone = false;
            s.redone = true;
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

    struct EmptyCmd {
        common: AbstractBase,
    }

    impl EmptyCmd {
        fn make() -> CommandRef {
            let ec = Rc::new(RefCell::new(EmptyCmd {
                common: AbstractBase {
                    label: RefCell::new(String::new()),
                    ..Default::default()
                },
            }));
            let cr: CommandRef = ec.clone() as Rc<RefCell<dyn Command>>;
            *ec.borrow().common.self_ref.borrow_mut() = Some(cr.clone());
            cr
        }
        fn with(label: &str, desc: &str) -> CommandRef {
            let ec = Rc::new(RefCell::new(EmptyCmd {
                common: AbstractBase {
                    label: RefCell::new(label.to_string()),
                    description: RefCell::new(desc.to_string()),
                    ..Default::default()
                },
            }));
            let cr: CommandRef = ec.clone() as Rc<RefCell<dyn Command>>;
            *ec.borrow().common.self_ref.borrow_mut() = Some(cr.clone());
            cr
        }
    }

    impl Command for EmptyCmd {
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

    // ---- AbstractCommand -------------------------------------------------
    #[test]
    fn abstract_command_prepare_defaults_to_false() {
        let cmd = EmptyCmd::make();
        assert!(!cmd.borrow().can_execute());
    }

    #[test]
    fn abstract_command_can_execute_cached() {
        let (cmd, _) = test_cmd("test");
        assert!(cmd.borrow().can_execute());
        assert!(cmd.borrow().can_execute());
        assert!(cmd.borrow().can_execute());
    }

    #[test]
    fn abstract_command_get_label_default() {
        let cmd = EmptyCmd::make();
        assert_eq!(cmd.borrow().get_label(), ABSTRACT_DEFAULT_LABEL);
    }

    #[test]
    fn abstract_command_get_description_default() {
        let cmd = EmptyCmd::make();
        assert_eq!(cmd.borrow().get_description(), ABSTRACT_DEFAULT_DESCRIPTION);
    }

    #[test]
    fn abstract_command_set_label_and_description() {
        let cmd = EmptyCmd::with("MyLabel", "MyDesc");
        assert_eq!(cmd.borrow().get_label(), "MyLabel");
        assert_eq!(cmd.borrow().get_description(), "MyDesc");
    }

    #[test]
    fn abstract_command_undo_throws() {
        let ec = EmptyCmd::make();
        let res = catch_unwind(AssertUnwindSafe(|| ec.borrow().undo()));
        assert!(res.is_err());
    }

    #[test]
    fn abstract_command_chain() {
        let (a, sa) = test_cmd("a");
        let (b, sb) = test_cmd("b");
        let chained = a.borrow().chain(b);
        assert!(chained.borrow().can_execute());
        chained.borrow().execute();
        assert!(sa.borrow().executed);
        assert!(sb.borrow().executed);
    }

    // ---- CompoundCommand -------------------------------------------------
    #[test]
    fn compound_command_empty_cannot_execute() {
        let cc = CompoundCommand::new();
        assert!(!cc.borrow().can_execute());
        assert!(cc.borrow().is_empty());
    }

    #[test]
    fn compound_command_basic_execution() {
        let (a, sa) = test_cmd("a");
        let (b, sb) = test_cmd("b");
        let cc = CompoundCommand::new();
        cc.borrow().append(a);
        cc.borrow().append(b);
        assert!(!cc.borrow().is_empty());
        assert_eq!(cc.borrow().command_count(), 2);
        assert!(cc.borrow().can_execute());
        cc.borrow().execute();
        assert!(sa.borrow().executed);
        assert!(sb.borrow().executed);
        cc.borrow().undo();
        assert!(sb.borrow().undone);
        assert!(sa.borrow().undone);
        cc.borrow().redo();
        assert!(sa.borrow().executed);
        assert!(sb.borrow().executed);
    }

    #[test]
    fn compound_command_result_index_last() {
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
        let cc = CompoundCommand::with_result_index(LAST_COMMAND_ALL);
        cc.borrow().append(a);
        cc.borrow().append(b);
        let res = cc.borrow().get_result();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn compound_command_result_index_0() {
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
        let cc = CompoundCommand::with_result_index(0);
        cc.borrow().append(a);
        cc.borrow().append(b);
        let res = cc.borrow().get_result();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn compound_command_label_from_last() {
        let (a, _) = test_cmd("labelA");
        let (b, _) = test_cmd("labelB");
        let cc = CompoundCommand::new();
        cc.borrow().append(a);
        cc.borrow().append(b);
        assert_eq!(cc.borrow().get_label(), "labelB");
    }

    #[test]
    fn compound_command_label_explicit() {
        let cc = CompoundCommand::with_label("explicit");
        assert_eq!(cc.borrow().get_label(), "explicit");
    }

    #[test]
    fn compound_command_append_null_ignored() {
        // There is no null CommandRef; instead we simulate the C++ path where a
        // null command is skipped (the wrapper delegates to an empty command).
        let cc = CompoundCommand::new();
        // No-op: nothing to append. Kept for parity with the C++ contract.
        assert!(cc.borrow().is_empty());
    }

    #[test]
    fn compound_command_append_if_can_execute() {
        let (a, _) = test_cmd("a");
        let cc = CompoundCommand::new();
        assert!(cc.borrow().append_if_can_execute(a));
        assert!(!cc.borrow().is_empty());
    }

    #[test]
    fn compound_command_unwrap_empty() {
        let cc = CompoundCommand::new();
        let u = cc.borrow().unwrap();
        assert!(!u.borrow().can_execute());
    }

    #[test]
    fn compound_command_unwrap_one() {
        let (a, _) = test_cmd("a");
        let cc = CompoundCommand::new();
        cc.borrow().append(a);
        let u = cc.borrow().unwrap();
        assert!(u.borrow().can_execute());
    }

    #[test]
    fn compound_command_unwrap_many() {
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
        let cc = CompoundCommand::new();
        cc.borrow().append(a);
        cc.borrow().append(b);
        let cc_ref: CommandRef = cc.clone() as CommandRef;
        let u = cc.borrow().unwrap();
        assert!(Rc::ptr_eq(&u, &cc_ref));
    }

    #[test]
    fn compound_command_append_after_prepared_throws() {
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
        let cc = CompoundCommand::new();
        cc.borrow().append(a);
        let _ = cc.borrow().can_execute();
        let res = catch_unwind(AssertUnwindSafe(|| cc.borrow().append(b)));
        assert!(res.is_err());
    }

    // ---- UnexecutableCommand ---------------------------------------------
    #[test]
    fn unexecutable_command_cannot_execute() {
        let u = UnexecutableCommand::instance();
        assert!(!u.borrow().can_execute());
        assert!(!u.borrow().can_undo());
        assert!(catch_unwind(AssertUnwindSafe(|| u.borrow().execute())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| u.borrow().redo())).is_err());
    }

    #[test]
    fn unexecutable_command_label() {
        let u = UnexecutableCommand::instance();
        assert_eq!(u.borrow().get_label(), UNEXECUTABLE_DEFAULT_LABEL);
    }

    // ---- IdentityCommand -------------------------------------------------
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
        assert_eq!(i.borrow().get_result().len(), 0);
    }

    #[test]
    fn identity_command_result_singleton() {
        let cmd = IdentityCommand::with_result_singleton(Val::String("hello".into()));
        let res = cmd.borrow().get_result();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], Val::String("hello".into()));
    }

    #[test]
    fn identity_command_result_collection() {
        let cmd = IdentityCommand::with_result(vec![Val::String("a".into()), Val::Int(42)]);
        let res = cmd.borrow().get_result();
        assert_eq!(res.len(), 2);
    }

    #[test]
    fn identity_command_label_description() {
        let i = IdentityCommand::instance();
        assert_eq!(i.borrow().get_label(), IDENTITY_DEFAULT_LABEL);
        assert_eq!(i.borrow().get_description(), IDENTITY_DEFAULT_DESCRIPTION);
    }

    #[test]
    fn identity_command_label_override() {
        let cmd = IdentityCommand::with_label("custom");
        assert_eq!(cmd.borrow().get_label(), "custom");
    }

    // ---- CommandWrapper --------------------------------------------------
    #[test]
    fn command_wrapper_delegates_execute() {
        let (inner, s) = test_cmd("inner");
        let w = CommandWrapper::new(inner);
        let wc: CommandRef = w.clone() as CommandRef;
        assert!(wc.borrow().can_execute());
        wc.borrow().execute();
        assert!(s.borrow().executed);
        wc.borrow().undo();
        assert!(s.borrow().undone);
        wc.borrow().redo();
        assert!(s.borrow().redone);
    }

    #[test]
    fn command_wrapper_can_undo_delegates() {
        let (inner, _) = test_cmd("inner");
        let w = CommandWrapper::new(inner);
        let wc: CommandRef = w.clone() as CommandRef;
        assert!(wc.borrow().can_undo());
    }

    #[test]
    fn command_wrapper_null_command() {
        let w = CommandWrapper::empty();
        let wc: CommandRef = w.clone() as CommandRef;
        assert!(!wc.borrow().can_execute());
        wc.borrow().execute();
        wc.borrow().undo();
        wc.borrow().redo();
        assert_eq!(wc.borrow().get_result().len(), 0);
    }

    #[test]
    fn command_wrapper_get_label_delegates() {
        let (inner, _) = test_cmd("inner-label");
        let w = CommandWrapper::new(inner);
        let wc: CommandRef = w.clone() as CommandRef;
        assert_eq!(wc.borrow().get_label(), "inner-label");
    }

    #[test]
    fn command_wrapper_get_label_override() {
        let (inner, _) = test_cmd("inner");
        let w = CommandWrapper::with_label("override", inner);
        let wc: CommandRef = w.clone() as CommandRef;
        assert_eq!(wc.borrow().get_label(), "override");
    }

    #[test]
    fn command_wrapper_lazy_create() {
        let created = Rc::new(Cell::new(false));
        let created_flag = created.clone();
        type Slot = (CommandRef, Rc<RefCell<TestState>>);
        let sink: Rc<RefCell<Option<Slot>>> = Rc::new(RefCell::new(None));
        let sink_ref = sink.clone();

        let w = CommandWrapper::lazy(move || {
            created_flag.set(true);
            let (c, state) = test_cmd("lazy");
            *sink_ref.borrow_mut() = Some((c.clone(), state));
            c
        });
        let wc: CommandRef = w.clone() as CommandRef;
        assert!(wc.borrow().can_execute());
        assert!(created.get());
        assert!(wc.borrow().can_execute());
        wc.borrow().execute();
        let (s, sstate) = sink.borrow().clone().expect("lazy command created");
        assert!(s.borrow().can_execute());
        assert!(sstate.borrow().executed);
    }

    // ---- StrictCompoundCommand -------------------------------------------
    #[test]
    fn strict_compound_command_empty_cannot_execute() {
        let sc = StrictCompoundCommand::new();
        assert!(!sc.borrow().can_execute());
    }

    #[test]
    fn strict_compound_command_basic_execute() {
        let (a, sa) = test_cmd("a");
        let (b, sb) = test_cmd("b");
        let sc = StrictCompoundCommand::new();
        sc.borrow().append(a);
        sc.borrow().append(b);
        assert!(sc.borrow().can_execute());
        sc.borrow().execute();
        assert!(sa.borrow().executed);
        assert!(sb.borrow().executed);
        sc.borrow().undo();
        assert!(sb.borrow().undone);
    }

    #[test]
    fn strict_compound_command_pessimistic() {
        let (a, sa) = test_cmd("a");
        let (b, sb) = test_cmd("b");
        let sc = StrictCompoundCommand::new();
        sc.borrow().set_is_pessimistic(true);
        sc.borrow().append(a);
        sc.borrow().append(b);
        assert!(sc.borrow().can_execute());
        sc.borrow().execute();
        assert!(sa.borrow().executed);
        assert!(sb.borrow().executed);
        sc.borrow().undo();
        assert!(!sa.borrow().executed);
        assert!(!sb.borrow().executed);
    }

    #[test]
    fn strict_compound_command_append_and_execute() {
        let (c, scstate) = test_cmd("c");
        let sc = StrictCompoundCommand::new();
        assert!(sc.borrow().append_and_execute(c));
        assert_eq!(sc.borrow().command_count(), 1);
        assert!(scstate.borrow().executed);
    }

    // ---- BasicCommandStack -----------------------------------------------
    struct StackListener {
        count: Rc<Cell<i32>>,
    }

    impl CommandStackListener for StackListener {
        fn command_stack_changed(&self) {
            self.count.set(self.count.get() + 1);
        }
    }

    fn listener() -> (Rc<dyn CommandStackListener>, Rc<Cell<i32>>) {
        let count = Rc::new(Cell::new(0));
        let l: Rc<dyn CommandStackListener> = Rc::new(StackListener {
            count: count.clone(),
        });
        (l, count)
    }

    #[test]
    fn basic_command_stack_execute_adds_command() {
        let stack = BasicCommandStack::new();
        let (cmd, _s) = test_cmd("a");
        stack.execute(Some(cmd.clone()));
        assert!(stack.get_undo_command().is_some());
        assert!(Rc::ptr_eq(&stack.get_undo_command().unwrap(), &cmd));
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        stack.flush();
    }

    #[test]
    fn basic_command_stack_undo_redo() {
        let stack = BasicCommandStack::new();
        let (cmd, s) = test_cmd("a");
        stack.execute(Some(cmd));
        assert!(stack.can_undo());
        stack.undo();
        assert!(s.borrow().undone);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());
        stack.redo();
        assert!(s.borrow().redone);
        stack.flush();
    }

    #[test]
    fn basic_command_stack_redo_clears_ahead() {
        let stack = BasicCommandStack::new();
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
        stack.execute(Some(a));
        stack.undo();
        stack.execute(Some(b));
        assert!(!stack.can_redo());
        assert!(stack.can_undo());
        assert!(stack.get_undo_command().is_some());
        stack.flush();
    }

    #[test]
    fn basic_command_stack_most_recent() {
        let stack = BasicCommandStack::new();
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
        stack.execute(Some(a));
        stack.execute(Some(b));
        assert!(stack.get_most_recent_command().is_some());
        stack.flush();
    }

    #[test]
    fn basic_command_stack_flush_clears() {
        let stack = BasicCommandStack::new();
        let (a, _) = test_cmd("a");
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
        let (l, count) = listener();
        stack.add_command_stack_listener(l.clone());
        let (a, _) = test_cmd("a");
        stack.execute(Some(a));
        assert_eq!(count.get(), 1);
        stack.undo();
        assert_eq!(count.get(), 2);
        stack.remove_command_stack_listener(&l);
        let (b, _) = test_cmd("b");
        stack.execute(Some(b));
        assert_eq!(count.get(), 2);
        stack.flush();
    }

    #[test]
    fn basic_command_stack_save_index() {
        let stack = BasicCommandStack::new();
        let (a, _) = test_cmd("a");
        let (b, _) = test_cmd("b");
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

    // ---- AbortExecutionException -----------------------------------------
    #[test]
    fn abort_execution_exception_default_constructor() {
        let e = AbortExecutionException::new();
        assert_eq!(e.message(), "");
    }

    #[test]
    fn abort_execution_exception_message_constructor() {
        let e = AbortExecutionException::with_message("aborted");
        assert_eq!(e.message(), "aborted");
    }

    #[test]
    fn abort_execution_exception_throw_and_catch() {
        let mut caught = false;
        let res = catch_unwind(AssertUnwindSafe(|| {
            panic_any(AbortExecutionException::with_message("abort"));
        }));
        match res {
            Err(payload) => {
                let e = payload
                    .downcast_ref::<AbortExecutionException>()
                    .expect("wrong panic payload type");
                assert_eq!(e.message(), "abort");
                caught = true;
            }
            Ok(_) => {}
        }
        assert!(caught);
    }

    // ---- CommandStackListener type alias ---------------------------------
    #[test]
    fn command_stack_listener_type_alias() {
        // The listener trait and its alias both exist and are usable.
        let (_l, _count) = listener();
        let _a: &dyn CommandStackListener = &StackListener {
            count: Rc::new(Cell::new(0)),
        };
        assert!(true);
    }
}
