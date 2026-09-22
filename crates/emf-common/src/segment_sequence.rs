//! EMF `SegmentSequence`, ported from C++ `emf-common/util/SegmentSequence`
//! (aligned to Java `org.eclipse.emf.common.util.SegmentSequence`).
//!
//! A memory-efficient, immutable sequence of delimiter-split string segments.
//! The C++ port uses shared_ptr + a global pool for hash-consing; in Rust the
//! type owns its segments and exposes the same query surface plus a `Builder`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Interning pool so identical `(delimiter, segments)` share storage.
struct Pool {
    map: HashMap<(String, Vec<String>), SegmentSequence>,
}

static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();

fn pool() -> &'static Mutex<Pool> {
    POOL.get_or_init(|| {
        Mutex::new(Pool {
            map: HashMap::new(),
        })
    })
}

/// A sequence of delimited string segments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SegmentSequence {
    delimiter: String,
    segments: Vec<String>,
}

impl SegmentSequence {
    /// Split `value` on `delimiter`.
    pub fn create(delimiter: &str, value: &str) -> SegmentSequence {
        let segments = split_segments(delimiter, value);
        intern(delimiter, segments)
    }

    /// An empty sequence for the delimiter.
    pub fn empty(delimiter: &str) -> SegmentSequence {
        intern(delimiter, Vec::new())
    }

    /// Build from an explicit list of segments (further split on delimiter).
    pub fn create_from_segments(delimiter: &str, segments: Vec<String>) -> SegmentSequence {
        let mut out = Vec::new();
        for s in segments {
            if delimiter.is_empty() {
                if !s.is_empty() {
                    out.push(s);
                }
            } else {
                out.extend(split_segments(delimiter, &s));
            }
        }
        intern(delimiter, out)
    }

    /// The delimiter.
    pub fn delimiter(&self) -> &str {
        &self.delimiter
    }

    /// Segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Segment at `index`.
    pub fn segment(&self, index: usize) -> Option<&str> {
        self.segments.get(index).map(String::as_str)
    }

    /// Last segment.
    pub fn last_segment(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// First segment.
    pub fn first_segment(&self) -> Option<&str> {
        self.segments.first().map(String::as_str)
    }

    /// String length of the joined representation.
    pub fn length(&self) -> usize {
        self.segments.join(&self.delimiter).len()
    }

    /// Append a single segment, returning a new sequence.
    pub fn append_segment(&self, segment: &str) -> SegmentSequence {
        let mut s = self.segments.clone();
        if self.delimiter.is_empty() && segment.is_empty() {
            return self.clone();
        }
        s.extend(split_segments(&self.delimiter, segment));
        intern(&self.delimiter, s)
    }

    /// Append a whole other sequence (its delimiter elements are merged onto ours).
    pub fn append(&self, other: &SegmentSequence) -> SegmentSequence {
        let mut s = self.segments.clone();
        s.extend(other.segments.iter().cloned());
        intern(&self.delimiter, s)
    }
}

impl std::fmt::Display for SegmentSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.segments.join(&self.delimiter))
    }
}

fn split_segments(delimiter: &str, value: &str) -> Vec<String> {
    if delimiter.is_empty() {
        // Character sequence when no delimiter.
        return value.chars().map(|c| c.to_string()).collect();
    }
    value.split(delimiter).map(String::from).collect()
}

fn intern(delimiter: &str, segments: Vec<String>) -> SegmentSequence {
    let mut g = pool().lock().unwrap();
    if let Some(existing) = g.map.get(&(delimiter.to_string(), segments.clone())) {
        return existing.clone();
    }
    let ss = SegmentSequence {
        delimiter: delimiter.to_string(),
        segments,
    };
    g.map
        .insert((ss.delimiter.clone(), ss.segments.clone()), ss.clone());
    ss
}

/// A mutable builder producing a `SegmentSequence`.
#[derive(Debug, Clone, Default)]
pub struct SegmentSequenceBuilder {
    delimiter: String,
    strings: Vec<String>,
}

impl SegmentSequenceBuilder {
    /// New builder.
    pub fn new(delimiter: &str) -> Self {
        Self {
            delimiter: delimiter.into(),
            strings: Vec::new(),
        }
    }

    /// Append a segment.
    pub fn append(mut self, s: impl Into<String>) -> Self {
        self.strings.push(s.into());
        self
    }

    /// Number of appended strings.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Whether no segments appended.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Build and intern.
    pub fn build(self) -> SegmentSequence {
        SegmentSequence::create_from_segments(&self.delimiter, self.strings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_delimiter() {
        let s = SegmentSequence::create("/", "a/b/c");
        assert_eq!(s.segment_count(), 3);
        assert_eq!(s.last_segment(), Some("c"));
        assert_eq!(s.to_string(), "a/b/c");
    }

    #[test]
    fn builder_joins() {
        let s = SegmentSequenceBuilder::new("/")
            .append("x")
            .append("y")
            .build();
        assert_eq!(s.to_string(), "x/y");
    }
}
