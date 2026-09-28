//! `MetaModelDescriptor` interface + `AbstractMetaModelDescriptor`.
//!
//! Port of C++ `emf/sphinx/metamodel/IMetaModelDescriptor.h` and
//! `AbstractMetaModelDescriptor.h` (aligned to Java
//! `org.eclipse.sphinx.emf.metamodel`).

use std::rc::Rc;

use emf_common::uri::Uri;

use super::version_data::MetaModelVersionData;

/// A meta-model descriptor: identifies one concrete EPackage family.
///
/// C++ models this as the abstract class `IMetaModelDescriptor`; Rust uses a
/// trait so several descriptor implementations can be registered together.
pub trait MetaModelDescriptor {
    /// The identifier (usually the namespace URI).
    fn identifier(&self) -> String;

    /// The namespace URI.
    fn namespace_uri(&self) -> Uri;

    /// The namespace as a string (default: `namespace_uri().to_string()`).
    fn namespace(&self) -> String {
        self.namespace_uri().to_string()
    }

    /// The display name.
    fn name(&self) -> String;

    /// The base descriptor, if any.
    fn base_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>>;

    /// The custom URI scheme, or `""`.
    fn custom_uri_scheme(&self) -> String;

    /// The ordinal (`-1` when unset).
    fn ordinal(&self) -> i32;

    /// The EPackage namespace-URI pattern (may be a regular expression).
    fn e_package_ns_uri_pattern(&self) -> String;

    /// Whether `ns` equals this descriptor's namespace.
    fn matches_namespace(&self, ns: &str) -> bool;

    /// Whether `ns` matches the EPackage namespace-URI pattern.
    fn matches_epackage_ns_uri_pattern(&self, ns: &str) -> bool;

    /// Equality by identifier. `None` never matches.
    fn equals(&self, other: Option<&dyn MetaModelDescriptor>) -> bool;

    /// The compatible resource-version descriptors.
    fn compatible_resource_version_descriptors(&self) -> Vec<Rc<dyn MetaModelDescriptor>>;
}

impl PartialEq for dyn MetaModelDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.equals(Some(other))
    }
}

/// The standard [`MetaModelDescriptor`] implementation.
///
/// Carries identifier / base namespace / EPackage nsURI postfix pattern /
/// name / version data, mirroring C++ `AbstractMetaModelDescriptor`.
#[derive(Clone)]
pub struct AbstractMetaModelDescriptor {
    identifier: String,
    base_namespace_uri: String,
    namespace_uri: Uri,
    e_package_ns_uri_postfix_pattern: String,
    name: String,
    version_data: Option<MetaModelVersionData>,
    custom_uri_scheme: String,
    compatible: Vec<Rc<dyn MetaModelDescriptor>>,
}

impl std::fmt::Debug for AbstractMetaModelDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbstractMetaModelDescriptor")
            .field("identifier", &self.identifier)
            .field("namespace", &self.namespace_uri.to_string())
            .field("name", &self.name)
            .finish()
    }
}

impl AbstractMetaModelDescriptor {
    /// Three-arg constructor: `identifier / namespace / name`.
    pub fn new(
        identifier: impl Into<String>,
        ns: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        let mut d = Self {
            identifier: identifier.into(),
            base_namespace_uri: ns.into(),
            namespace_uri: Uri::new(),
            e_package_ns_uri_postfix_pattern: String::new(),
            name: name.into(),
            version_data: None,
            custom_uri_scheme: String::new(),
            compatible: Vec::new(),
        };
        d.init_namespace();
        d
    }

    /// Four-arg constructor (multi EPackage):
    /// `identifier / baseNamespace / ePackageNsURIPostfixPattern / name`.
    pub fn with_pattern(
        identifier: impl Into<String>,
        base_namespace: impl Into<String>,
        e_package_ns_uri_postfix_pattern: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        let mut d = Self {
            identifier: identifier.into(),
            base_namespace_uri: base_namespace.into(),
            namespace_uri: Uri::new(),
            e_package_ns_uri_postfix_pattern: e_package_ns_uri_postfix_pattern.into(),
            name: name.into(),
            version_data: None,
            custom_uri_scheme: String::new(),
            compatible: Vec::new(),
        };
        d.init_namespace();
        d
    }

    /// Three-arg constructor with version data:
    /// `identifier / baseNamespace / versionData`. The name is taken from the
    /// version data.
    pub fn with_version(
        identifier: impl Into<String>,
        base_namespace: impl Into<String>,
        version_data: MetaModelVersionData,
    ) -> Self {
        let mut d = Self {
            identifier: identifier.into(),
            base_namespace_uri: base_namespace.into(),
            namespace_uri: Uri::new(),
            e_package_ns_uri_postfix_pattern: String::new(),
            name: String::new(),
            version_data: Some(version_data),
            custom_uri_scheme: String::new(),
            compatible: Vec::new(),
        };
        d.init_namespace();
        if d.name.is_empty() {
            if let Some(vd) = &d.version_data {
                d.name = vd.name().to_string();
            }
        }
        d
    }

    /// The identifier.
    pub fn get_identifier(&self) -> &str {
        &self.identifier
    }

    /// Set the identifier.
    pub fn set_identifier(&mut self, v: impl Into<String>) {
        self.identifier = v.into();
    }

    /// The namespace URI.
    pub fn get_namespace_uri(&self) -> &Uri {
        &self.namespace_uri
    }

    /// Set the namespace URI (also resets the base namespace).
    pub fn set_namespace_uri(&mut self, v: Uri) {
        self.base_namespace_uri = v.to_string();
        self.namespace_uri = v;
    }

    /// The display name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the display name.
    pub fn set_name(&mut self, v: impl Into<String>) {
        self.name = v.into();
    }

    /// The base namespace URI (without version postfix).
    pub fn base_namespace_uri(&self) -> &str {
        &self.base_namespace_uri
    }

    /// The version data, if any.
    pub fn version_data(&self) -> Option<&MetaModelVersionData> {
        self.version_data.as_ref()
    }

    /// Replace the version data, syncing the namespace and pattern.
    pub fn set_version_data(&mut self, v: MetaModelVersionData) {
        self.name = v.name().to_string();
        self.version_data = Some(v);
        self.init_namespace();
    }

    /// Set the custom URI scheme.
    pub fn set_custom_uri_scheme(&mut self, v: impl Into<String>) {
        self.custom_uri_scheme = v.into();
    }

    /// Add a compatible resource-version descriptor.
    pub fn add_compatible_resource_version_descriptor(&mut self, d: Rc<dyn MetaModelDescriptor>) {
        self.compatible.push(d);
    }

    /// Hash code by identifier (aligned to C++ `hashCode`).
    pub fn hash_code(&self) -> i32 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.identifier.hash(&mut h);
        h.finish() as i32
    }

    /// Compute the namespace: `base + ("/" + version postfix if any)`.
    fn init_namespace(&mut self) {
        let mut ns = self.base_namespace_uri.clone();
        if let Some(vd) = &self.version_data {
            if !vd.ns_postfix().is_empty() {
                ns.push('/');
                ns.push_str(vd.ns_postfix());
            }
        }
        self.namespace_uri = Uri::parse(ns);
    }

    /// Compute the effective EPackage namespace-URI pattern string.
    fn pattern_string(&self) -> String {
        let mut s = self.base_namespace_uri.clone();
        if !self.e_package_ns_uri_postfix_pattern.is_empty() {
            s.push('/');
            s.push_str(&self.e_package_ns_uri_postfix_pattern);
        } else if let Some(vd) = &self.version_data {
            if !vd.e_package_ns_uri_postfix_pattern().is_empty() {
                s.push('/');
                s.push_str(vd.e_package_ns_uri_postfix_pattern());
            }
        }
        s
    }
}

impl MetaModelDescriptor for AbstractMetaModelDescriptor {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn namespace_uri(&self) -> Uri {
        self.namespace_uri.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn base_descriptor(&self) -> Option<Rc<dyn MetaModelDescriptor>> {
        self.version_data
            .as_ref()
            .and_then(|vd| vd.base_descriptor().cloned())
    }

    fn custom_uri_scheme(&self) -> String {
        self.custom_uri_scheme.clone()
    }

    fn ordinal(&self) -> i32 {
        self.version_data
            .as_ref()
            .map(|vd| vd.ordinal())
            .unwrap_or(-1)
    }

    fn e_package_ns_uri_pattern(&self) -> String {
        self.pattern_string()
    }

    fn matches_namespace(&self, ns: &str) -> bool {
        self.namespace_uri.to_string() == ns
    }

    fn matches_epackage_ns_uri_pattern(&self, ns: &str) -> bool {
        let pattern = self.pattern_string();
        if pattern.is_empty() {
            return false;
        }
        regex_lite::full_match(&pattern, ns)
    }

    fn equals(&self, other: Option<&dyn MetaModelDescriptor>) -> bool {
        match other {
            Some(o) => self.identifier == o.identifier(),
            None => false,
        }
    }

    fn compatible_resource_version_descriptors(&self) -> Vec<Rc<dyn MetaModelDescriptor>> {
        self.compatible.clone()
    }
}

/// A minimal full-match regular-expression engine covering the subset used by
/// EPackage namespace-URI patterns.
///
/// C++ uses `std::regex_match`; to keep `emf-sphinx` dependency-free we support
/// the constructs that meta-model descriptors actually use: literals, `.`,
/// character classes `[abc]` / `[a-z]` / `[^...]`, escapes, and the `*` / `+` /
/// `?` quantifiers. Matching is a full match (the whole input must be consumed).
mod regex_lite {
    #[derive(Clone)]
    enum Atom {
        Literal(char),
        Any,
        Class {
            negated: bool,
            ranges: Vec<(char, char)>,
            chars: Vec<char>,
        },
    }

    impl Atom {
        fn matches(&self, c: char) -> bool {
            match self {
                Atom::Literal(l) => *l == c,
                Atom::Any => true,
                Atom::Class {
                    negated,
                    ranges,
                    chars,
                } => {
                    let inside =
                        chars.contains(&c) || ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi);
                    inside != *negated
                }
            }
        }
    }

    struct Token {
        atom: Atom,
        min: usize,
        max: Option<usize>,
    }

    fn parse(pattern: &str) -> Option<Vec<Token>> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut tokens: Vec<Token> = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let atom = match chars[i] {
                '\\' => {
                    i += 1;
                    if i >= chars.len() {
                        return None;
                    }
                    Atom::Literal(chars[i])
                }
                '.' => Atom::Any,
                '[' => {
                    i += 1;
                    let negated = i < chars.len() && chars[i] == '^';
                    if negated {
                        i += 1;
                    }
                    let mut ranges = Vec::new();
                    let mut singles = Vec::new();
                    let mut closed = false;
                    while i < chars.len() {
                        if chars[i] == ']' {
                            closed = true;
                            break;
                        }
                        let lo = chars[i];
                        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
                            ranges.push((lo, chars[i + 2]));
                            i += 3;
                        } else {
                            singles.push(lo);
                            i += 1;
                        }
                    }
                    if !closed {
                        return None;
                    }
                    Atom::Class {
                        negated,
                        ranges,
                        chars: singles,
                    }
                }
                '(' | ')' | '|' | '^' | '$' => return None,
                c => Atom::Literal(c),
            };
            i += 1;
            // Postfix quantifier.
            let (min, max) = match chars.get(i) {
                Some('*') => {
                    i += 1;
                    (0, None)
                }
                Some('+') => {
                    i += 1;
                    (1, None)
                }
                Some('?') => {
                    i += 1;
                    (0, Some(1))
                }
                _ => (1, Some(1)),
            };
            tokens.push(Token { atom, min, max });
        }
        Some(tokens)
    }

    fn matches_from(tokens: &[Token], ti: usize, text: &[char], pos: usize) -> bool {
        if ti == tokens.len() {
            return pos == text.len();
        }
        let tok = &tokens[ti];
        // How many consecutive characters from `pos` this atom can consume.
        let max = tok.max.unwrap_or(usize::MAX);
        let mut matched = 0usize;
        while matched < max && pos + matched < text.len() && tok.atom.matches(text[pos + matched]) {
            matched += 1;
        }
        // Greedy with backtracking: try the longest repetition first.
        let mut count = matched;
        loop {
            if count >= tok.min && matches_from(tokens, ti + 1, text, pos + count) {
                return true;
            }
            if count == 0 {
                return false;
            }
            count -= 1;
        }
    }

    /// Whether `text` fully matches `pattern`.
    pub fn full_match(pattern: &str, text: &str) -> bool {
        let tokens = match parse(pattern) {
            Some(t) => t,
            None => return false,
        };
        let chars: Vec<char> = text.chars().collect();
        matches_from(&tokens, 0, &chars, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_matches_numeric_versions() {
        let d =
            AbstractMetaModelDescriptor::with_pattern("urn:my.mm", "urn:my.mm", "v[0-9]+", "My");
        assert_eq!(d.e_package_ns_uri_pattern(), "urn:my.mm/v[0-9]+");
        assert!(d.matches_epackage_ns_uri_pattern("urn:my.mm/v1"));
        assert!(d.matches_epackage_ns_uri_pattern("urn:my.mm/v42"));
        assert!(!d.matches_epackage_ns_uri_pattern("urn:my.mm/stable"));
        assert!(!d.matches_epackage_ns_uri_pattern("urn:other.mm/v1"));
    }
}
