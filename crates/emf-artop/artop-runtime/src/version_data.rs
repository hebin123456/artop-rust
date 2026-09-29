//! AUTOSAR metamodel version number (port of C++ `AutosarMetaModelVersionData`).
//!
//! Aligned to Java `org.artop.aal.common.metamodel.AutosarMetaModelVersionData`.

use std::fmt;

/// `major.minor.revision` version triple of an AUTOSAR metamodel release.
///
/// Drives schema file naming: 4.x releases up to 4.3 use a 5-digit code
/// (`AUTOSAR_00042.xsd`), 4.4+ use the dotted form (`AUTOSAR_4-4-8.xsd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AutosarMetaModelVersionData {
    major: i32,
    minor: i32,
    revision: i32,
}

impl AutosarMetaModelVersionData {
    /// Last 4.x minor version using the *old* schema naming (`4.3`).
    pub const LAST_OLD_4X_MINOR_VERSION: i32 = 3;
    /// First 4.x minor version using the *new* schema naming (`4.4`).
    pub const FIRST_NEW_4X_MINOR_VERSION: i32 = 4;

    /// Construct from an explicit triple.
    pub fn new(major: i32, minor: i32, revision: i32) -> Self {
        Self {
            major,
            minor,
            revision,
        }
    }

    /// Parse `"4.4.8"` / `"4.4"` / `"4"` (C++
    /// `createFromCanonicalVersionNumberString`, which throws `invalid_argument`).
    ///
    /// Missing parts default to `0`; an empty string or a part without a leading
    /// number is rejected. The leading number of each dot-separated part is used
    /// (matching C++ `std::stoi` partial-parse semantics).
    pub fn create_from_canonical_version_number_string(s: &str) -> Result<Self, String> {
        if s.is_empty() {
            return Err("AutosarMetaModelVersionData: empty version string".to_string());
        }
        let mut parts = [0i32; 3];
        let mut part_count = 0usize;
        let mut cur = String::new();
        for c in s.chars() {
            if c == '.' {
                if part_count >= 3 {
                    return Err(format!(
                        "AutosarMetaModelVersionData: too many parts in {s}"
                    ));
                }
                parts[part_count] = parse_leading_int(&cur)
                    .ok_or_else(|| format!("AutosarMetaModelVersionData: bad number in {s}"))?;
                part_count += 1;
                cur.clear();
            } else {
                cur.push(c);
            }
        }
        if part_count >= 3 {
            return Err(format!(
                "AutosarMetaModelVersionData: too many parts in {s}"
            ));
        }
        if !cur.is_empty() {
            parts[part_count] = parse_leading_int(&cur)
                .ok_or_else(|| format!("AutosarMetaModelVersionData: bad number in {s}"))?;
            part_count += 1;
        }
        if part_count == 0 {
            return Err(format!("AutosarMetaModelVersionData: empty version {s}"));
        }
        Ok(Self::new(
            parts[0],
            if part_count > 1 { parts[1] } else { 0 },
            if part_count > 2 { parts[2] } else { 0 },
        ))
    }

    /// Major component.
    pub fn major(&self) -> i32 {
        self.major
    }
    /// Minor component.
    pub fn minor(&self) -> i32 {
        self.minor
    }
    /// Revision component.
    pub fn revision(&self) -> i32 {
        self.revision
    }

    /// Whether `(major, minor)` uses the new 4.4+ schema naming
    /// (Java `isNewVersion(int, int)`).
    pub fn is_new_version_for(major: i32, minor: i32) -> bool {
        major == 4 && minor >= Self::FIRST_NEW_4X_MINOR_VERSION
    }

    /// Whether this version uses the new 4.4+ schema naming.
    pub fn is_new_version(&self) -> bool {
        Self::is_new_version_for(self.major, self.minor)
    }

    /// Pack into one integer: `major << 24 | minor << 16 | revision`
    /// (Java `getCanonicalVersionNumber`).
    pub fn canonical_version_number(&self) -> i32 {
        (self.major << 24) | (self.minor << 16) | (self.revision & 0xFFFF)
    }

    /// Schema file-name version segment: `"4-4-8"` for new versions (using
    /// `separator`), `"00042"` for old ones (Java `getSchemaVersionNumberString`).
    pub fn schema_version_number_string(&self, separator: &str) -> String {
        if self.is_new_version() {
            format!(
                "{}{separator}{}{separator}{}",
                self.major, self.minor, self.revision
            )
        } else {
            // 4.0 -> 40, 4.2 -> 42, ... zero-padded to 3 digits after "00".
            let code = 40 + self.minor;
            format!("00{code:03}")
        }
    }
}

impl fmt::Display for AutosarMetaModelVersionData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.revision)
    }
}

/// Parse the leading integer of `s` like C++ `std::stoi` (optional sign, then
/// digits; trailing non-digits ignored). `None` when there is no leading digit.
fn parse_leading_int(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut sign = 1i64;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        if bytes[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let start = i;
    let mut value: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value * 10 + i64::from(bytes[i] - b'0');
        i += 1;
    }
    if i == start {
        return None;
    }
    Some((sign * value) as i32)
}

/// Newtype alias kept for readability at call sites.
pub type VersionData = AutosarMetaModelVersionData;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_and_partial() {
        let v = AutosarMetaModelVersionData::create_from_canonical_version_number_string("4.4.8")
            .unwrap();
        assert_eq!((v.major(), v.minor(), v.revision()), (4, 4, 8));
        let v = AutosarMetaModelVersionData::create_from_canonical_version_number_string("4.0")
            .unwrap();
        assert_eq!((v.major(), v.minor(), v.revision()), (4, 0, 0));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(
            AutosarMetaModelVersionData::create_from_canonical_version_number_string("").is_err()
        );
        assert!(
            AutosarMetaModelVersionData::create_from_canonical_version_number_string("4.4.8.1")
                .is_err()
        );
        assert!(
            AutosarMetaModelVersionData::create_from_canonical_version_number_string("x").is_err()
        );
    }

    #[test]
    fn schema_names_old_and_new() {
        assert_eq!(
            AutosarMetaModelVersionData::new(4, 4, 8).schema_version_number_string("-"),
            "4-4-8"
        );
        assert_eq!(
            AutosarMetaModelVersionData::new(4, 2, 1).schema_version_number_string(""),
            "00042"
        );
        assert!(AutosarMetaModelVersionData::new(4, 4, 8).is_new_version());
        assert!(!AutosarMetaModelVersionData::new(4, 2, 1).is_new_version());
    }
}
