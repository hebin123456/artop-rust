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
    /// Split `value` on `delimiter` (C++ `create(delimiter, value)`).
    ///
    /// - An empty `value` yields an empty sequence.
    /// - An empty `delimiter` keeps the whole `value` as a *single* segment.
    /// - Otherwise the value is split on the delimiter, keeping empty segments.
    pub fn create(delimiter: &str, value: &str) -> SegmentSequence {
        if value.is_empty() {
            return SegmentSequence::empty(delimiter);
        }
        if delimiter.is_empty() {
            return intern(delimiter, vec![value.to_string()]);
        }
        intern(
            delimiter,
            value.split(delimiter).map(String::from).collect(),
        )
    }

    /// An empty sequence for the delimiter.
    pub fn empty(delimiter: &str) -> SegmentSequence {
        intern(delimiter, Vec::new())
    }

    /// Build from an explicit list of segments (further split on delimiter,
    /// C++ `create(delimiter, segments)`).
    pub fn create_from_segments(delimiter: &str, segments: Vec<String>) -> SegmentSequence {
        intern(delimiter, split_segments(delimiter, &segments))
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

    /// Append a single segment, returning a new sequence (C++ `append(string)`).
    pub fn append_segment(&self, segment: &str) -> SegmentSequence {
        if self.segments.is_empty() {
            return SegmentSequence::create(&self.delimiter, segment);
        }
        if segment.contains(&self.delimiter) {
            let sub = SegmentSequence::create(&self.delimiter, segment);
            return self.append(&sub);
        }
        let mut s = self.segments.clone();
        s.push(segment.to_string());
        intern(&self.delimiter, s)
    }

    /// Append a whole other sequence (C++ `append(SegmentSequence)`). When the
    /// delimiters differ, `other`'s segments are re-split onto this delimiter.
    pub fn append(&self, other: &SegmentSequence) -> SegmentSequence {
        if self.segments.is_empty() {
            if self.delimiter == other.delimiter {
                return other.clone();
            }
            return intern(
                &self.delimiter,
                split_segments(&self.delimiter, &other.segments),
            );
        }
        let mut s = self.segments.clone();
        if self.delimiter != other.delimiter {
            s.extend(split_segments(&self.delimiter, &other.segments));
        } else {
            s.extend(other.segments.iter().cloned());
        }
        intern(&self.delimiter, s)
    }

    /// Append multiple segments, each split on the delimiter, returning a new
    /// sequence (C++ `append(vector<string>)`).
    pub fn append_segments(&self, segments: &[&str]) -> SegmentSequence {
        let owned: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
        if self.segments.is_empty() {
            return SegmentSequence::create_from_segments(&self.delimiter, owned);
        }
        let mut s = self.segments.clone();
        s.extend(split_segments(&self.delimiter, &owned));
        intern(&self.delimiter, s)
    }

    /// Sub-range of segments, half-open `[from, to)` as a view.
    pub fn sub_segments(&self, from: usize, to: usize) -> &[String] {
        &self.segments[from..to]
    }

    /// Character at `index` in the joined representation.
    pub fn char_at(&self, index: usize) -> Option<char> {
        self.to_string().chars().nth(index)
    }

    /// Sub-sequence of the joined representation, `[from, to)` as a string.
    pub fn sub_sequence(&self, from: usize, to: usize) -> String {
        let s = self.to_string();
        s.get(from..to).unwrap_or("").to_string()
    }
}

impl std::fmt::Display for SegmentSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.segments.join(&self.delimiter))
    }
}

/// C++ `splitSegments(delimiter, segments, length)`: split a *list* of
/// segments. With an empty delimiter, empty segments are dropped and the rest
/// are kept verbatim; otherwise each segment containing the delimiter is split
/// on it, while segments without it are kept as-is.
fn split_segments(delimiter: &str, segments: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(segments.len());
    if delimiter.is_empty() {
        for s in segments {
            if !s.is_empty() {
                out.push(s.clone());
            }
        }
        return out;
    }
    for s in segments {
        if s.contains(delimiter) {
            out.extend(s.split(delimiter).map(String::from));
        } else {
            out.push(s.clone());
        }
    }
    out
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

    /// Reverse the order of appended strings.
    pub fn reverse(mut self) -> Self {
        self.strings.reverse();
        self
    }
}

impl std::fmt::Display for SegmentSequenceBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Semi-joined rendering of the appended strings. Implemented directly
        // in `Display` (rather than an inherent `to_string`) so it does not
        // shadow the blanket `ToString` impl.
        f.write_str(&self.strings.join(&self.delimiter))
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

    #[test]
    fn to_string_simple() {
        let s = SegmentSequence::create("/", "a/b/c");
        assert_eq!(s.to_string(), "a/b/c");
    }

    #[test]
    fn empty_has_zero_count() {
        let s = SegmentSequence::empty("/");
        assert_eq!(s.segment_count(), 0);
        assert_eq!(s.to_string(), "");
    }

    #[test]
    fn append_string_produces_new_sequence() {
        let s = SegmentSequence::create("/", "a").append_segment("b");
        assert_eq!(s.to_string(), "a/b");
        assert_eq!(s.segment_count(), 2);
    }

    #[test]
    fn append_string_with_delimiter() {
        let s = SegmentSequence::create("/", "a/b");
        assert_eq!(s.to_string(), "a/b");
    }

    #[test]
    fn single_segment_sequence() {
        let s = SegmentSequence::create("/", "only");
        assert_eq!(s.segment_count(), 1);
        assert_eq!(s.first_segment(), Some("only"));
        assert_eq!(s.last_segment(), Some("only"));
    }

    #[test]
    fn length_and_count() {
        let s = SegmentSequence::create("/", "a/b/c");
        assert_eq!(s.segment_count(), 3);
        assert_eq!(s.length(), "a/b/c".len());
    }

    #[test]
    fn segment_access() {
        let s = SegmentSequence::create("/", "a/b/c");
        assert_eq!(s.segment(0), Some("a"));
        assert_eq!(s.segment(1), Some("b"));
        assert_eq!(s.segment(2), Some("c"));
        assert_eq!(s.segment(3), None);
    }

    #[test]
    fn no_delimiter_is_single_segment() {
        let s = SegmentSequence::create("", "abc");
        assert_eq!(s.segment_count(), 1);
        assert_eq!(s.segments(), ["abc"]);
    }

    #[test]
    fn empty_value_is_empty_sequence() {
        let s = SegmentSequence::create("/", "");
        assert_eq!(s.segment_count(), 0);
        assert_eq!(s.to_string(), "");
    }

    #[test]
    fn empty_delimiter_append_keeps_whole_segment() {
        let s = SegmentSequence::create("", "x").append_segment("ab");
        assert_eq!(s.segment_count(), 2);
        assert_eq!(s.segments(), ["x", "ab"]);
        assert_eq!(s.to_string(), "xab");
    }

    #[test]
    fn empty_delimiter_single() {
        let s = SegmentSequence::create("", "z");
        assert_eq!(s.segment_count(), 1);
        assert_eq!(s.to_string(), "z");
    }

    #[test]
    fn different_delimiters_are_distinct() {
        let slash = SegmentSequence::create("/", "a/b");
        let dot = SegmentSequence::create(".", "a/b");
        assert_eq!(slash.to_string(), "a/b");
        assert_eq!(dot.to_string(), "a/b");
        assert_eq!(slash.delimiter(), "/");
        assert_eq!(dot.delimiter(), ".");
        assert_ne!(slash, dot); // hash-consing keeps them separate by delimiter
    }

    #[test]
    fn hashcode_consistent() {
        let s1 = SegmentSequence::create("/", "a/b/c");
        let s2 = SegmentSequence::create("/", "a/b/c");
        // Interned: equal sequences are the same value (structural eq).
        assert_eq!(s1, s2);
    }

    #[test]
    fn append_chain() {
        let seq = SegmentSequence::empty("/");
        let seq = seq
            .append_segment("a")
            .append_segment("b")
            .append_segment("c");
        assert_eq!(seq.segment_count(), 3);
        assert_eq!(seq.to_string(), "a/b/c");
    }

    #[test]
    fn append_different_delimiter() {
        let a = SegmentSequence::create("/", "a/b");
        let b = SegmentSequence::create(".", "c.d");
        let c = a.append(&b);
        // a's delimiter is "/"; b's segments have no "/", so no extra split -> a/b/c/d
        assert_eq!(c.delimiter(), "/");
        assert_eq!(c.to_string(), "a/b/c/d");
    }

    #[test]
    fn append_segment_sequence() {
        let a = SegmentSequence::create("/", "a/b");
        let b = SegmentSequence::create("/", "c/d");
        let c = a.append(&b);
        assert_eq!(c.segment_count(), 4);
        assert_eq!(c.to_string(), "a/b/c/d");
    }

    #[test]
    fn append_vector() {
        let seq = SegmentSequence::create("/", "a");
        let seq2 = seq.append_segments(&["b", "c", "d"]);
        assert_eq!(seq2.segment_count(), 4);
        assert_eq!(seq2.to_string(), "a/b/c/d");
    }

    #[test]
    fn builder_append_char() {
        let b = SegmentSequenceBuilder::new(".");
        let b = b.append('a').append('b').append('c');
        assert_eq!(b.to_string(), "a.b.c");
    }

    #[test]
    fn builder_basic() {
        let b = SegmentSequenceBuilder::new("/");
        let seq = b.append("a").append("b").append("c").build();
        assert_eq!(seq.segment_count(), 3);
        assert_eq!(seq.to_string(), "a/b/c");
    }

    #[test]
    fn builder_reverse() {
        let b = SegmentSequenceBuilder::new("/");
        let seq = b.append("a").append("b").append("c").reverse().build();
        assert_eq!(seq.to_string(), "c/b/a");
    }

    #[test]
    fn char_sequence() {
        let seq = SegmentSequence::create("/", "foo/bar");
        assert_eq!(seq.char_at(0), Some('f'));
        assert_eq!(seq.char_at(3), Some('/'));
        assert_eq!(seq.char_at(4), Some('b'));
        assert_eq!(seq.char_at(100), None);
        assert_eq!(seq.sub_sequence(0, 3), "foo");
        assert_eq!(seq.sub_sequence(4, 7), "bar");
    }

    #[test]
    fn create_empty_has_zero_count() {
        let seq = SegmentSequence::empty("/");
        assert_eq!(seq.segment_count(), 0);
        assert_eq!(seq.to_string(), "");
        assert_eq!(seq.length(), 0);
    }

    #[test]
    fn create_split_segment() {
        let seq = SegmentSequence::create_from_segments("/", vec!["foo/bar".into(), "baz".into()]);
        assert_eq!(seq.segment_count(), 3);
        assert_eq!(seq.to_string(), "foo/bar/baz");
    }

    #[test]
    fn create_vararg() {
        let seq = SegmentSequence::create_from_segments(
            "/",
            vec!["alpha".into(), "beta".into(), "gamma".into()],
        );
        assert_eq!(seq.segment_count(), 3);
        assert_eq!(seq.segment(1), Some("beta"));
        assert_eq!(seq.to_string(), "alpha/beta/gamma");
    }

    #[test]
    fn pool_intern() {
        let a = SegmentSequence::create("/", "foo/bar");
        let b = SegmentSequence::create("/", "foo/bar");
        // Interned pool: identical (delimiter, segments) yield the same structural value.
        assert_eq!(a, b);
    }

    #[test]
    fn segments_copy() {
        let seq = SegmentSequence::create("/", "x/y/z");
        let segs: Vec<String> = seq.segments().to_vec();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "x");
        assert_eq!(segs[1], "y");
        assert_eq!(segs[2], "z");
    }

    #[test]
    fn segments_list_view() {
        let seq = SegmentSequence::create("/", "p/q/r");
        let list = seq.segments();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "p");
        assert_eq!(list[2], "r");
    }

    #[test]
    fn sub_segments() {
        let seq = SegmentSequence::create("/", "a/b/c/d");
        let sub = seq.sub_segments(1, 3);
        assert_eq!(sub.len(), 2);
        assert_eq!(sub[0], "b");
        assert_eq!(sub[1], "c");
    }

    #[test]
    fn sub_segments_list_view() {
        let seq = SegmentSequence::create("/", "a/b/c/d");
        let list = seq.sub_segments(1, 3);
        assert_eq!(list.len(), 2);
        assert_eq!(list.get(0).map(String::as_str), Some("b"));
        assert_eq!(list.get(1).map(String::as_str), Some("c"));
    }
}
