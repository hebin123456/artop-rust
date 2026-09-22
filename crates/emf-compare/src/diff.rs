//! `Diff` — a single detected difference between two (or three) `EObject`s
//! (aligned to Java `org.eclipse.emf.compare.Diff`, C++ `emf-compare/Diff.h`).
//!
//! Ported over the `emf-common` reflection surface so it stays artop-agnostic.
//! Object identity is `Rc<RefCell<dyn EObject>>` ([`ObjectRef`]); a `Diff`
//! remembers which [`Match`](crate::comparison::Match) it belongs to.

use emf_common::value::{ObjectRef, Val};

/// Difference kind (aligned to Java `DifferenceKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// An element was added.
    Add,
    /// An element was removed.
    Delete,
    /// A feature value changed.
    Change,
    /// An element moved within an ordered many-valued reference.
    Move,
}

/// Diff subtype (aligned to Java `Diff` subclass hierarchy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffType {
    /// Whole-object ADD/DELETE/CHANGE (no feature).
    ElementChange,
    /// An `EAttribute` CHANGE.
    AttributeChange,
    /// An `EReference` CHANGE / ADD / DELETE / MOVE.
    ReferenceChange,
}

/// Which side produced the difference (aligned to Java `DifferenceSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceSource {
    Left,
    Right,
}

/// Merge state of a diff (aligned to Java `DifferenceState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceState {
    Pending,
    Merged,
    Discarded,
}

/// Whether two object handles are "semantically equal" (aligned to Java
/// `DefaultEqualityHelper`). Non-proxy objects compare by pointer identity.
pub fn e_object_equals(a: &ObjectRef, b: &ObjectRef) -> bool {
    std::rc::Rc::ptr_eq(a, b)
}

/// A single difference between two objects' feature values.
#[derive(Debug, Clone)]
pub struct Diff {
    kind: DiffKind,
    type_: DiffType,
    attribute_name: String,
    left: Option<ObjectRef>,
    right: Option<ObjectRef>,
    source: DifferenceSource,
    state: DifferenceState,
    old_index: i32,
    new_index: i32,
    old_value: Option<Val>,
    new_value: Option<Val>,
    /// Index of the owning `Match` in the `Comparison`, when known.
    match_index: Option<usize>,
}

impl Default for Diff {
    fn default() -> Self {
        Self {
            kind: DiffKind::Change,
            type_: DiffType::ElementChange,
            attribute_name: String::new(),
            left: None,
            right: None,
            source: DifferenceSource::Right,
            state: DifferenceState::Pending,
            old_index: -1,
            new_index: -1,
            old_value: None,
            new_value: None,
            match_index: None,
        }
    }
}

impl Diff {
    /// New diff with kind + attribute name.
    pub fn new(kind: DiffKind, attribute_name: impl Into<String>) -> Self {
        Self {
            kind,
            attribute_name: attribute_name.into(),
            ..Self::default()
        }
    }

    /// Setter `setLeft`.
    pub fn with_left(mut self, o: ObjectRef) -> Self {
        self.set_left(Some(o));
        self
    }
    /// Setter `setRight`.
    pub fn with_right(mut self, o: ObjectRef) -> Self {
        self.set_right(Some(o));
        self
    }
    /// Setter `setKind`.
    pub fn with_kind(mut self, k: DiffKind) -> Self {
        self.kind = k;
        self
    }
    /// Setter `setType`.
    pub fn with_type(mut self, t: DiffType) -> Self {
        self.type_ = t;
        self
    }
    /// Setter `setSource`.
    pub fn with_source(mut self, s: DifferenceSource) -> Self {
        self.source = s;
        self
    }
    /// Setter `setOldValue`.
    pub fn with_old_value(mut self, v: Val) -> Self {
        self.old_value = Some(v);
        self
    }
    /// Setter `setNewValue`.
    pub fn with_new_value(mut self, v: Val) -> Self {
        self.new_value = Some(v);
        self
    }
    /// Setter `setOldIndex` (MOVE).
    pub fn with_old_index(mut self, i: i32) -> Self {
        self.old_index = i;
        self
    }
    /// Setter `setNewIndex` (MOVE).
    pub fn with_new_index(mut self, i: i32) -> Self {
        self.new_index = i;
        self
    }

    /// `getKind`.
    pub fn kind(&self) -> DiffKind {
        self.kind
    }
    /// `setKind`.
    pub fn set_kind(&mut self, k: DiffKind) {
        self.kind = k;
    }
    /// `getType`.
    pub fn type_(&self) -> DiffType {
        self.type_
    }
    /// `setType`.
    pub fn set_type(&mut self, t: DiffType) {
        self.type_ = t;
    }
    /// `getAttributeName`.
    pub fn attribute_name(&self) -> &str {
        &self.attribute_name
    }
    /// `setAttributeName`.
    pub fn set_attribute_name(&mut self, s: impl Into<String>) {
        self.attribute_name = s.into();
    }
    /// `getLeft`.
    pub fn left(&self) -> Option<&ObjectRef> {
        self.left.as_ref()
    }
    /// `setLeft`.
    pub fn set_left(&mut self, o: Option<ObjectRef>) {
        self.left = o;
    }
    /// `getRight`.
    pub fn right(&self) -> Option<&ObjectRef> {
        self.right.as_ref()
    }
    /// `setRight`.
    pub fn set_right(&mut self, o: Option<ObjectRef>) {
        self.right = o;
    }
    /// `getSource`.
    pub fn source(&self) -> DifferenceSource {
        self.source
    }
    /// `setSource`.
    pub fn set_source(&mut self, s: DifferenceSource) {
        self.source = s;
    }
    /// `getState`.
    pub fn state(&self) -> DifferenceState {
        self.state
    }
    /// `setState`.
    pub fn set_state(&mut self, s: DifferenceState) {
        self.state = s;
    }
    /// MOVE `oldIndex`.
    pub fn old_index(&self) -> i32 {
        self.old_index
    }
    /// `setOldIndex`.
    pub fn set_old_index(&mut self, i: i32) {
        self.old_index = i;
    }
    /// MOVE `newIndex`.
    pub fn new_index(&self) -> i32 {
        self.new_index
    }
    /// `setNewIndex`.
    pub fn set_new_index(&mut self, i: i32) {
        self.new_index = i;
    }
    /// `getOldValue`.
    pub fn old_value(&self) -> Option<&Val> {
        self.old_value.as_ref()
    }
    /// `setOldValue`.
    pub fn set_old_value(&mut self, v: Val) {
        self.old_value = Some(v);
    }
    /// `getNewValue`.
    pub fn new_value(&self) -> Option<&Val> {
        self.new_value.as_ref()
    }
    /// `setNewValue`.
    pub fn set_new_value(&mut self, v: Val) {
        self.new_value = Some(v);
    }
    /// The owning match index in the `Comparison`, if set.
    pub fn match_index(&self) -> Option<usize> {
        self.match_index
    }
    /// `setMatch` — record the owning match index.
    pub fn set_match_index(&mut self, i: Option<usize>) {
        self.match_index = i;
    }
}
