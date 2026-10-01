//! A dependency-free, non-cryptographic hasher for the hot reflective maps.
//!
//! The ARXML load/save paths perform millions of lookups keyed by short strings
//! (feature names, class names) and small integers (feature ids, object
//! pointers). `std::collections::HashMap` defaults to SipHash-1-3, which is
//! DoS-resistant but several times slower than needed here — these maps are
//! internal to the serializers and never fed attacker-controlled key streams.
//!
//! This is a compact FxHash-style multiply-rotate hasher (the same family as
//! `rustc`'s `FxHasher`): one `rotl` + one `wrapping_mul` per 8 bytes. It is
//! deliberately *not* used for anything that must resist hash-flooding.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// The multiply-rotate hasher (FxHash).
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut b = bytes;
        while b.len() >= 8 {
            let mut chunk = [0u8; 8];
            chunk.copy_from_slice(&b[..8]);
            self.add(u64::from_le_bytes(chunk));
            b = &b[8..];
        }
        if b.len() >= 4 {
            let mut chunk = [0u8; 4];
            chunk.copy_from_slice(&b[..4]);
            self.add(u32::from_le_bytes(chunk) as u64);
            b = &b[4..];
        }
        for &byte in b {
            self.add(byte as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
}

/// `BuildHasher` for [`FxHasher`].
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// A `HashMap` using [`FxHasher`].
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// A `HashSet` using [`FxHasher`].
pub type FxHashSet<T> = HashSet<T, FxBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_round_trip() {
        let mut m: FxHashMap<String, u32> = FxHashMap::default();
        for i in 0..1000u32 {
            m.insert(format!("feature-{i}"), i);
        }
        for i in 0..1000u32 {
            assert_eq!(m.get(&format!("feature-{i}")), Some(&i));
        }
        assert_eq!(m.len(), 1000);
    }
}
