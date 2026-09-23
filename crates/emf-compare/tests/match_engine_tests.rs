//! Port of `MatchEngineTests.cpp` — `MatchEngine` configuration + both-null.
//!
//! Rust API mapping:
//! - `getSimilarityThreshold` -> `similarity_threshold()`
//! - `getUseIdentifierMatcher` -> `use_identifier_matcher()`
//! - `setSimilarityThreshold`   -> `set_similarity_threshold()`
//! - `me.match(nullptr, nullptr, comp)` -> `me.match_2way(None, None, &mut comp)`

use emf_compare::comparison::Comparison;
use emf_compare::match_engine::MatchEngine;

/// C++ `MatchEngine_Threshold_Defaults`: default similarity threshold 1.0 and
/// identifier matcher disabled.
#[test]
fn match_engine_threshold_defaults() {
    let me = MatchEngine::new();
    assert_eq!(me.similarity_threshold(), 1.0);
    assert!(!me.use_identifier_matcher());
}

/// C++ `MatchEngine_SetThreshold`.
#[test]
fn match_engine_set_threshold() {
    let mut me = MatchEngine::new();
    me.set_similarity_threshold(0.5);
    assert_eq!(me.similarity_threshold(), 0.5);
}

/// C++ `MatchEngine_BothNull_ReturnsNoMatch`: matching two `null` roots yields
/// no `Match`.
#[test]
fn match_engine_both_null_returns_no_match() {
    let mut comp = Comparison::new();
    let mut me = MatchEngine::new();
    me.match_2way(None, None, &mut comp);
    assert_eq!(comp.matches().len(), 0);
}