//! `Comparison` model + top-level `compare` entry points (aligned to Java
//! `org.eclipse.emf.compare.{Match, Comparison, Conflict}`, C++
//! `emf-compare/Comparison.h`).
//!
//! All object handles are [`ObjectRef`] (`Rc<RefCell<dyn EObject>>`), and
//! identity is pointer identity. `Match`es, `Diff`s, `Conflict`s,
//! `Equivalence`s and `Dependency`s are owned by the [`Comparison`].

use crate::diff::Diff;
use emf_common::value::ObjectRef;

/// `MatchKind` (aligned to Java `MatchKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Identical,
    Different,
    AbsentLeft,
    AbsentRight,
}

/// `ConflictKind` (aligned to Java `ConflictKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    Real,
    Pseudo,
}

/// A `Match`: the correspondence between left/right (and 3-way origin) objects.
#[derive(Debug)]
pub struct Match {
    left: Option<ObjectRef>,
    right: Option<ObjectRef>,
    origin: Option<ObjectRef>,
    kind: MatchKind,
    similarity: f64,
    /// Indices into the owning `Comparison`'s `differences` vector.
    diffs: Vec<usize>,
}

impl Match {
    /// A new match.
    pub fn new(
        left: Option<ObjectRef>,
        right: Option<ObjectRef>,
        kind: MatchKind,
        similarity: f64,
    ) -> Self {
        Self {
            left,
            right,
            origin: None,
            kind,
            similarity,
            diffs: Vec::new(),
        }
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
    /// `getOrigin` (3-way).
    pub fn origin(&self) -> Option<&ObjectRef> {
        self.origin.as_ref()
    }
    /// `setOrigin`.
    pub fn set_origin(&mut self, o: Option<ObjectRef>) {
        self.origin = o;
    }
    /// `getKind`.
    pub fn kind(&self) -> MatchKind {
        self.kind
    }
    /// `setKind`.
    pub fn set_kind(&mut self, k: MatchKind) {
        self.kind = k;
    }
    /// `getSimilarity`.
    pub fn similarity(&self) -> f64 {
        self.similarity
    }
    /// `setSimilarity`.
    pub fn set_similarity(&mut self, s: f64) {
        self.similarity = s;
    }
    /// The diff indices owned by this match.
    pub fn diff_indices(&self) -> &[usize] {
        &self.diffs
    }
    /// Append a diff (by index into the comparison's difference vector).
    pub fn push_diff(&mut self, idx: usize) {
        if !self.diffs.contains(&idx) {
            self.diffs.push(idx);
        }
    }
}

/// A `Conflict`: two sides changed the same feature relative to origin.
#[derive(Debug)]
pub struct Conflict {
    kind: ConflictKind,
    /// Indices into the owning `Comparison`'s `differences`.
    diffs: Vec<usize>,
}

impl Default for Conflict {
    fn default() -> Self {
        Self {
            kind: ConflictKind::Real,
            diffs: Vec::new(),
        }
    }
}

impl Conflict {
    /// New conflict.
    pub fn new(kind: ConflictKind) -> Self {
        Self {
            kind,
            diffs: Vec::new(),
        }
    }
    /// `getKind`.
    pub fn kind(&self) -> ConflictKind {
        self.kind
    }
    /// `setKind`.
    pub fn set_kind(&mut self, k: ConflictKind) {
        self.kind = k;
    }
    /// Diff indices owned by this conflict.
    pub fn diff_indices(&self) -> &[usize] {
        &self.diffs
    }
    /// Append a diff index.
    pub fn push_diff(&mut self, idx: usize) {
        if !self.diffs.contains(&idx) {
            self.diffs.push(idx);
        }
    }
}

/// An `Equivalence`: two `Match`es are linked via a non-containment reference.
#[derive(Debug, Default)]
pub struct Equivalence {
    /// Match indices into the owning `Comparison`.
    matches: Vec<usize>,
}

impl Equivalence {
    /// Add a match by index.
    pub fn add_match(&mut self, idx: usize) {
        if !self.matches.contains(&idx) {
            self.matches.push(idx);
        }
    }
    /// The member match indices.
    pub fn matches(&self) -> &[usize] {
        &self.matches
    }
}

/// A `Dependency`: an ordering constraint between two `Diff`s (`source` depends
/// on `target`, so `target` must be merged first). Indices into `differences`.
#[derive(Debug, Clone, Copy)]
pub struct Dependency {
    source: usize,
    target: usize,
}

impl Dependency {
    /// New dependency.
    pub fn new(source: usize, target: usize) -> Self {
        Self { source, target }
    }
    /// The depending diff index.
    pub fn source(&self) -> usize {
        self.source
    }
    /// The depended-on diff index.
    pub fn target(&self) -> usize {
        self.target
    }
}

/// An external identifier provider (aligned to Java `IdentifiableUtil` /
/// C++ `IdentifierProvider`). Returns a non-empty ID to match strictly by ID.
pub type IdentifierProvider = dyn Fn(&ObjectRef) -> String + 'static;

/// A `Comparison`: the result set of one match+diff over object graphs.
#[derive(Debug, Default)]
pub struct Comparison {
    matches: Vec<Match>,
    differences: Vec<Diff>,
    conflicts: Vec<Conflict>,
    equivalences: Vec<Equivalence>,
    dependencies: Vec<Dependency>,
    three_way: bool,
}

impl Comparison {
    /// A fresh, empty comparison.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a match, returning its index.
    pub fn add_match(
        &mut self,
        left: Option<ObjectRef>,
        right: Option<ObjectRef>,
        kind: MatchKind,
        similarity: f64,
    ) -> usize {
        self.matches.push(Match::new(left, right, kind, similarity));
        self.matches.len() - 1
    }

    /// Append a diff, returning its index.
    pub fn add_diff(&mut self, diff: Diff) -> usize {
        self.differences.push(diff);
        self.differences.len() - 1
    }

    /// Append a diff and associate it with a match (by index).
    pub fn add_diff_to_match(&mut self, diff: Diff, match_idx: usize) -> usize {
        let mut diff = diff;
        diff.set_match_index(Some(match_idx));
        let idx = self.add_diff(diff);
        if match_idx < self.matches.len() {
            self.matches[match_idx].push_diff(idx);
        }
        idx
    }

    /// All matches.
    pub fn matches(&self) -> &[Match] {
        &self.matches
    }
    /// Mutable matches.
    pub fn matches_mut(&mut self) -> &mut Vec<Match> {
        &mut self.matches
    }
    /// A match by index.
    pub fn match_at(&self, i: usize) -> Option<&Match> {
        self.matches.get(i)
    }

    /// All differences.
    pub fn differences(&self) -> &[Diff] {
        &self.differences
    }
    /// Mutable differences.
    pub fn differences_mut(&mut self) -> &mut Vec<Diff> {
        &mut self.differences
    }
    /// A diff by index.
    pub fn diff_at(&self, i: usize) -> Option<&Diff> {
        self.differences.get(i)
    }

    /// All conflicts.
    pub fn conflicts(&self) -> &[Conflict] {
        &self.conflicts
    }
    /// Mutable conflicts.
    pub fn conflicts_mut(&mut self) -> &mut Vec<Conflict> {
        &mut self.conflicts
    }

    /// All equivalences.
    pub fn equivalences(&self) -> &[Equivalence] {
        &self.equivalences
    }
    /// Mutable equivalences.
    pub fn equivalences_mut(&mut self) -> &mut Vec<Equivalence> {
        &mut self.equivalences
    }

    /// All dependencies.
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }
    /// Mutable dependencies.
    pub fn dependencies_mut(&mut self) -> &mut Vec<Dependency> {
        &mut self.dependencies
    }

    /// Whether this is a three-way comparison.
    pub fn is_three_way(&self) -> bool {
        self.three_way
    }
    /// `setThreeWay`.
    pub fn set_three_way(&mut self, b: bool) {
        self.three_way = b;
    }

    /// Clear all result collections, keeping the comparison reusable.
    pub fn clear(&mut self) {
        self.matches.clear();
        self.differences.clear();
        self.conflicts.clear();
        self.equivalences.clear();
        self.dependencies.clear();
    }
}

/// Top-level convenience: 2-way match + diff + equivalence.
pub fn compare(left: Option<&ObjectRef>, right: Option<&ObjectRef>) -> Comparison {
    let mut comp = Comparison::new();
    crate::match_engine::do_match(left, right, None, &mut comp);
    crate::diff_engine::do_diff(&mut comp);
    crate::equivalence_engine::compute_equivalences(&mut comp);
    comp
}

/// Top-level convenience: 2-way match + diff + equivalence with a custom
/// identifier provider.
pub fn compare_with_provider<P>(
    left: Option<&ObjectRef>,
    right: Option<&ObjectRef>,
    provider: P,
) -> Comparison
where
    P: Fn(&ObjectRef) -> String + 'static,
{
    let mut me = crate::match_engine::MatchEngine::new();
    me.set_identifier_provider(Box::new(provider));
    let mut comp = Comparison::new();
    me.match_2way(left, right, &mut comp);
    crate::diff_engine::do_diff(&mut comp);
    crate::equivalence_engine::compute_equivalences(&mut comp);
    comp
}

/// Top-level convenience: 3-way match + diff + conflict + equivalence.
pub fn compare3(
    left: Option<&ObjectRef>,
    right: Option<&ObjectRef>,
    origin: Option<&ObjectRef>,
) -> Comparison {
    let mut comp = Comparison::new();
    crate::match_engine::do_match(left, right, origin, &mut comp);
    crate::diff_engine::do_diff(&mut comp);
    crate::conflict_detector::detect_conflicts(&mut comp);
    crate::equivalence_engine::compute_equivalences(&mut comp);
    comp
}

// Re-export the diff type so callers can build diffs conveniently.
pub use crate::diff::{e_object_equals, Diff as TypedDiff, DiffKind as TypedDiffKind};
pub use crate::diff::{DifferenceSource, DifferenceState};
