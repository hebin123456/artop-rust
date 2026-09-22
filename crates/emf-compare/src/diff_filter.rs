//! `DiffFilter` — post-process filters that veto differences before they are
//! attached to a comparison (aligned to Java `org.eclipse.emf.compare.diff.
//! DiffFilter` and its `MinimalDiffFilter`).
//!
//! The engine runs every computed diff through the active filters; a diff
//! rejected by any filter is dropped, so the downstream merge/UI never sees it.

use crate::comparison::Comparison;
use crate::diff::{Diff, DiffKind, DiffType};

/// Whether a diff survives filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterVote {
    /// Keep the diff.
    Keep,
    /// Reject the diff — it is dropped from the comparison.
    Reject,
}

/// A single filter over computed differences.
pub trait DiffFilter {
    /// Decide whether `diff` (in `comparison`) stays. Returning [`FilterVote::Keep`]
    /// accepts it; [`FilterVote::Reject`] removes it.
    fn filter(&self, diff: &Diff, comparison: &Comparison) -> FilterVote;
}

/// Default `MinimalDiffFilter`: keeps every diff except whole-object changes
/// that are also entirely unset (nothing meaningful to compare), mirroring the
/// behaviour of EMF's minimal filter.
pub struct MinimalDiffFilter;

impl DiffFilter for MinimalDiffFilter {
    fn filter(&self, diff: &Diff, _comparison: &Comparison) -> FilterVote {
        // No-op element changes with no value and no object carry no signal.
        if matches!(diff.kind(), DiffKind::Change)
            && diff.type_() == DiffType::ElementChange
            && diff.old_value().is_none()
            && diff.new_value().is_none()
        {
            return FilterVote::Reject;
        }
        FilterVote::Keep
    }
}

/// Run `filters` over the current diffs of `comparison`, dropping rejected ones
/// in place and keeping the remaining indices stable for downstream engines.
pub fn filter_diffs(comparison: &mut Comparison, filters: &[&dyn DiffFilter]) {
    if filters.is_empty() {
        return;
    }
    // Snapshot the diffs we started with (count before removal).
    let total = comparison.differences().len();
    let mut keep = Vec::with_capacity(total);
    for i in 0..total {
        let diff = {
            // Cloned so we can inspect while removing.
            let d = comparison.diff_at(i);
            match d {
                Some(d) => d.clone(),
                None => continue,
            }
        };
        let mut ok = true;
        for f in filters {
            if f.filter(&diff, comparison) == FilterVote::Reject {
                ok = false;
                break;
            }
        }
        if ok {
            keep.push(diff);
        }
    }
    // Rebuild.
    let diffs = comparison.differences_mut();
    diffs.clear();
    diffs.extend(keep);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::Diff;

    #[test]
    fn minimal_filter_rejects_empty_element_change() {
        let mut comp = Comparison::new();
        let empty = Diff::new(DiffKind::Change, "").with_type(DiffType::ElementChange);
        comp.add_diff(empty);
        let real = Diff::new(DiffKind::Change, "name").with_type(DiffType::AttributeChange);
        comp.add_diff(real);

        let f = MinimalDiffFilter;
        let filters: [&dyn DiffFilter; 1] = [&f];
        filter_diffs(&mut comp, &filters);

        assert_eq!(comp.differences().len(), 1);
        assert_eq!(comp.diff_at(0).unwrap().type_(), DiffType::AttributeChange);
    }
}
