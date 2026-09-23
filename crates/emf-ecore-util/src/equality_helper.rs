//! `EcoreUtil.EqualityHelper` (EMF `org.eclipse.emf.ecore.util.EcoreUtil$EqualityHelper`,
//! C++ `emf-ecore-util/EqualityHelper`).
//!
//! A lightweight equality + hash helper: `equals` delegates to the deep
//! [`crate::ecore_util::equals`], and `hash_code` produces a deterministic
//! FNV-1a hash of a value for use as a (feeble but deterministic) map key.

use emf_common::value::{ObjectRef, Val};

/// Structural equality + hash helper over reflective objects / values.
pub struct EqualityHelper;

impl EqualityHelper {
    /// New, stateless helper.
    pub fn new() -> Self {
        EqualityHelper
    }

    /// Compare two objects with deep value equality (EMF
    /// `EqualityHelper.equals(EObject, EObject)`); delegates to
    /// [`crate::ecore_util::equals`]. Two `None`s are equal.
    pub fn equals(&self, a: Option<&ObjectRef>, b: Option<&ObjectRef>) -> bool {
        crate::ecore_util::equals(a, b)
    }

    /// A deterministic hash of a value; a `None` (null / unset) hashes to `0.0`
    /// so it can never collide with a real value (EMF
    /// `EqualityHelper.hashCode(Object)`).
    pub fn hash_code(&self, value: Option<&Val>) -> f64 {
        match value {
            None => 0.0,
            Some(v) => fnv1a(v.describe().as_bytes()) as f64,
        }
    }
}

/// FNV-1a 64-bit hash.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_hashes_to_zero() {
        assert_eq!(EqualityHelper::new().hash_code(None), 0.0);
    }

    #[test]
    fn string_hashes_non_zero() {
        let h = EqualityHelper::new().hash_code(Some(&Val::string("a")));
        assert_ne!(h, 0.0);
    }
}