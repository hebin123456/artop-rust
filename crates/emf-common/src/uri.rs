//! URI value type, ported from C++ `emf-common/URI` (aligned to Java
//! `org.eclipse.emf.common.util.URI`).
//!
//! Fields mirror the C++ struct: scheme, `authority` handle, `path`, `query`,
//! `fragment`, `device`, plus flags for scheme-opaque and archive URIs.

use std::fmt;

/// A URI parsed per EMF rules.
#[derive(Debug, Clone, PartialEq)]
pub struct Uri {
    scheme: String,
    opaque: String,
    device: String,
    authority: String,
    path: String,
    fragment: String,
    query: String,
    scheme_opaque: bool,
    is_archive: bool,
    has_authority: bool,
}

impl Uri {
    /// Empty URI.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a URI by parsing the given string.
    pub fn parse(s: impl Into<String>) -> Self {
        Self::default().parse_inner(s.into())
    }

    /// Create a `file:`, ensuring the `//` authority segment like the C++ port.
    pub fn create_file_uri(path: &str) -> Self {
        Self::default().file_uri(path.into())
    }

    /// Create a `platform:/...` URI.
    pub fn create_platform_uri(path: &str) -> Self {
        let mut u = Self::default();
        u.scheme = "platform".into();
        u.path = if path.is_empty() {
            "/".into()
        } else if path.starts_with('/') {
            path.into()
        } else {
            format!("/{path}")
        };
        u
    }

    fn parse_inner(mut self, s: String) -> Self {
        let mut s = s;
        if s.is_empty() {
            return self;
        }
        if let Some(fp) = s.find('#') {
            self.fragment = s[fp + 1..].to_string();
            s.truncate(fp);
        }
        if let Some(qp) = s.find('?') {
            self.query = s[qp + 1..].to_string();
            s.truncate(qp);
        }
        if let Some(cp) = s.find(':') {
            let mut is_scheme = cp > 0 && s.as_bytes()[0].is_ascii_alphabetic();
            for b in s.as_bytes()[1..cp].iter() {
                if !(b.is_ascii_alphanumeric() || *b == b'+' || *b == b'-' || *b == b'.') {
                    is_scheme = false;
                    break;
                }
            }
            if is_scheme {
                self.scheme = s[..cp].to_string();
                s = s[cp + 1..].to_string();
                if s.len() >= 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[1] == b'/' {
                    self.has_authority = true;
                    let ap = s[2..].find('/');
                    let authority = match ap {
                        Some(i) => s[2..2 + i].to_string(),
                        None => s[2..].to_string(),
                    };
                    let at = authority.find('@');
                    let host_info = match at {
                        Some(i) => authority[i + 1..].to_string(),
                        None => authority.clone(),
                    };
                    let bang = host_info.find("!/");
                    if let Some(bang) = bang {
                        self.is_archive = true;
                        let mut first = host_info[..bang].to_string();
                        let rest = host_info[bang + 1..].to_string();
                        while first.len() >= 2 && &first[..2] == "//" {
                            first.drain(..2);
                        }
                        while first.len() >= 2 && &first[..2] == "//" {
                            first.drain(..2);
                        }
                        if first.len() >= 2
                            && first.as_bytes()[0].is_ascii_alphabetic()
                            && first.as_bytes()[1] == b':'
                        {
                            self.device = first[..2].to_string();
                            first.drain(..2);
                        }
                        self.authority = host_info[..bang].to_string();
                        if !first.is_empty() && !first.starts_with('/') {
                            first.insert(0, '/');
                        }
                        self.path = format!("{first}{rest}");
                    } else {
                        let mut host_info = host_info;
                        while host_info.len() >= 2 && &host_info[..2] == "//" {
                            host_info.drain(..2);
                        }
                        if host_info.len() >= 2
                            && host_info.as_bytes()[0].is_ascii_alphabetic()
                            && host_info.as_bytes()[1] == b':'
                        {
                            self.device = host_info[..2].to_string();
                            host_info.drain(..2);
                        }
                        self.authority = host_info;
                        self.path.clear();
                    }
                    if let Some(ap) = s[2..].find('/') {
                        self.path.push_str(&s[2 + ap..]);
                    }
                } else {
                    self.scheme_opaque = true;
                    self.opaque = s;
                }
            } else {
                self.path = s;
            }
        } else {
            self.path = s;
        }
        self
    }

    /// Windows/unix/relative path handling shared by `create_file_uri`.
    fn file_uri(mut self, p: String) -> Self {
        if p.is_empty() {
            return self;
        }
        // Windows absolute (C:/...)
        if p.len() >= 2 && p.as_bytes()[1] == b':' {
            self.scheme = "file".into();
            self.has_authority = true;
            self.device = p[..2].to_string();
            self.path = p[2..].to_string();
            if !self.path.is_empty() && !self.path.starts_with('/') {
                self.path.insert(0, '/');
            }
            return self;
        }
        // Unix absolute
        if p.starts_with('/') {
            self.scheme = "file".into();
            self.has_authority = true;
            self.path = p;
            return self;
        }
        // Relative path: resolve against cwd. Fall back to scheme-less relative.
        if let Ok(cwd) = std::env::current_dir() {
            let abs = cwd.join(&p);
            let path = abs.to_string_lossy().to_string();
            if !path.starts_with('/') {
                self.scheme.clear();
                self.path = p;
                return self;
            }
            self.scheme = "file".into();
            self.has_authority = true;
            self.path = path;
            return self;
        }
        self.scheme.clear();
        self.path = p;
        self
    }

    // ==== queryers ====
    pub fn is_empty(&self) -> bool {
        self.scheme.is_empty() && self.path.is_empty() && self.opaque.is_empty()
    }
    pub fn is_file(&self) -> bool {
        self.scheme == "file"
    }
    pub fn is_platform(&self) -> bool {
        self.scheme == "platform"
    }
    pub fn is_archive(&self) -> bool {
        self.is_archive
    }
    pub fn is_relative(&self) -> bool {
        // 对齐 C++: (!scheme && path空) || (!path空且不以'/'开头)
        let no_scheme_empty_path = !self.has_scheme() && self.path.is_empty();
        let rel_path = !self.path.is_empty() && !self.path.starts_with('/');
        no_scheme_empty_path || rel_path
    }
    pub fn has_relative_path(&self) -> bool {
        let p = &self.path;
        if p.is_empty() {
            return false;
        }
        if p == "." || p == ".." {
            return true;
        }
        for (i, ch) in p.char_indices() {
            if ch == '.' && (i == 0 || p.as_bytes()[i - 1] == b'/') {
                let next = p.as_bytes().get(i + 1);
                if let Some(n) = next {
                    if *n == b'/' || *n == b'.' {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }
        false
    }
    pub fn is_hierarchical(&self) -> bool {
        !self.scheme_opaque
    }
    pub fn has_absolute_path(&self) -> bool {
        !self.path.is_empty() && self.path.starts_with('/')
    }
    pub fn has_scheme(&self) -> bool {
        !self.scheme.is_empty()
    }

    /// Whether the path has no leading `/` (a relative path).
    pub fn path_is_relative(&self) -> bool {
        !self.path.is_empty() && !self.path.starts_with('/')
    }

    // ==== accessors ====
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    pub fn opaque_part(&self) -> &str {
        &self.opaque
    }
    pub fn device(&self) -> &str {
        &self.device
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn fragment(&self) -> &str {
        &self.fragment
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn authority(&self) -> &str {
        &self.authority
    }
    pub fn is_scheme_opaque(&self) -> bool {
        self.scheme_opaque
    }

    fn parse_segments(&self, p: &str) -> Vec<String> {
        p.split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Path segments (dropping the leading empty segment), per the C++ port.
    pub fn segments(&self) -> Vec<String> {
        if self.scheme_opaque {
            return Vec::new();
        }
        self.parse_segments(&self.path)
    }

    pub fn segment(&self, index: usize) -> Option<String> {
        self.segments().into_iter().nth(index)
    }

    pub fn to_file_path(&self) -> String {
        if self.scheme == "file" || self.scheme.is_empty() {
            let mut p = self.path.clone();
            if self.device.len() == 2 && self.device.starts_with('/') {
                p = format!("{}{}", self.device, p);
            }
            p
        } else {
            self.to_string()
        }
    }

    /// Append a fragment, returning a new URI.
    pub fn append_fragment(&self, f: &str) -> Uri {
        let mut r = self.clone();
        r.fragment = f.into();
        r
    }

    /// Append a path segment.
    pub fn append_segment(&self, seg: &str) -> Uri {
        let mut r = self.clone();
        if !r.path.is_empty() && !r.path.ends_with('/') {
            r.path.push('/');
        }
        r.path.push_str(seg);
        r
    }

    /// Drop the fragment.
    pub fn trim_fragment(&self) -> Uri {
        let mut r = self.clone();
        r.fragment.clear();
        r
    }

    /// Trim the last `count` path segments (EMF `trimSegments`).
    pub fn trim_segments(&self, count: u32) -> Uri {
        if count < 1 {
            return self.clone();
        }
        let mut r = self.clone();
        let mut p = r.path.clone();
        if p.starts_with('/') {
            p.remove(0);
        }
        let segs: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
        if (count as usize) >= segs.len() {
            r.path.clear();
        } else {
            let keep = segs.len() - count as usize;
            let joined = segs[..keep].join("/");
            r.path = format!("/{joined}");
        }
        r
    }

    /// Resolve a relative URI against a base URI (EMF `URI.resolve`).
    pub fn resolve(&self, base: &Uri) -> Uri {
        if !self.is_relative() {
            return self.clone();
        }
        if !base.is_hierarchical() || base.is_relative() {
            return self.clone();
        }
        let mut result = Uri::default();
        result.scheme = base.scheme.clone();
        result.device = base.device.clone();
        result.has_authority = base.has_authority;
        result.authority = base.authority.clone();

        if !self.path.is_empty() {
            let base_segs = base.parse_segments(&base.path);
            let mut new_segs: Vec<String> = Vec::new();
            let mut first = true;
            for seg in self.parse_segments(&self.path) {
                if seg == "." {
                    continue;
                } else if seg == ".." {
                    if let Some(last) = new_segs.last() {
                        if last != ".." {
                            new_segs.pop();
                        } else if !first || !base.has_absolute_path() {
                            new_segs.push(seg);
                        }
                    } else if !first || !base.has_absolute_path() {
                        new_segs.push(seg);
                    }
                } else {
                    new_segs.push(seg);
                }
                first = false;
            }
            let mut path = String::new();
            if base.has_absolute_path() && new_segs.is_empty() {
                path.push('/');
            } else if base.has_absolute_path() && !base_segs.is_empty() {
                for seg in &base_segs[..base_segs.len() - 1] {
                    path.push('/');
                    path.push_str(seg);
                }
            }
            for seg in &new_segs {
                path.push('/');
                path.push_str(seg);
            }
            result.path = path;
        } else {
            result.path = base.path.clone();
        }
        result.query = self.query.clone();
        result.fragment = self.fragment.clone();
        result
    }

    /// Deresolve an absolute URI relative to a base URI (EMF `URI.deresolve`).
    pub fn deresolve(&self, base: &Uri) -> Uri {
        if self.is_relative() || !base.is_hierarchical() || base.is_relative() {
            return self.clone();
        }
        if self.scheme != base.scheme {
            return self.clone();
        }
        if self.device != base.device {
            return self.clone();
        }
        let base_segs = base.parse_segments(&base.path);
        let my_segs = self.parse_segments(&self.path);
        let mut common = 0usize;
        let min = base_segs.len().min(my_segs.len());
        while common < min && base_segs[common] == my_segs[common] {
            common += 1;
        }
        let mut out = String::new();
        for _ in common..base_segs.len() {
            out.push_str("../");
        }
        for i in common..my_segs.len() {
            out.push_str(&my_segs[i]);
            if i + 1 < my_segs.len() {
                out.push('/');
            }
        }
        let mut result = Uri::default();
        result.path = out;
        result.query = self.query.clone();
        result.fragment = self.fragment.clone();
        result
    }

    /// Is this a usable base URI?
    pub fn is_base(&self) -> bool {
        self.is_hierarchical() && !self.is_relative()
    }

    /// Effective base URI used when serializing references.
    pub fn append_raw_segment(&self, _raw: &str) -> Uri {
        self.clone()
    }
}

impl Default for Uri {
    fn default() -> Self {
        Self {
            scheme: String::new(),
            opaque: String::new(),
            device: String::new(),
            authority: String::new(),
            path: String::new(),
            fragment: String::new(),
            query: String::new(),
            scheme_opaque: false,
            is_archive: false,
            has_authority: false,
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        if !self.scheme.is_empty() {
            out.push_str(&self.scheme);
            out.push(':');
        }
        if self.scheme_opaque {
            out.push_str(&self.opaque);
        } else {
            if self.has_authority {
                out.push_str("//");
            }
            if !self.authority.is_empty() {
                out.push_str(&self.authority);
            }
            out.push_str(&self.path);
        }
        if !self.query.is_empty() {
            out.push('?');
            out.push_str(&self.query);
        }
        if !self.fragment.is_empty() {
            out.push('#');
            out.push_str(&self.fragment);
        }
        f.write_str(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_path() {
        let u = Uri::parse("foo/bar.xmi");
        assert_eq!(u.path(), "foo/bar.xmi");
        assert!(u.is_relative());
    }

    #[test]
    fn parses_file_with_authority() {
        let u = Uri::parse("file:///tmp/a.xmi");
        assert!(u.is_file());
        assert_eq!(u.scheme(), "file");
        assert!(u.has_authority);
        assert_eq!(u.path(), "/tmp/a.xmi");
    }

    #[test]
    fn roundtrip_uri() {
        let u = Uri::parse("platform:/resource/m/app.arxml#/frag");
        assert_eq!(u.to_string(), "platform:/resource/m/app.arxml#/frag");
        assert_eq!(u.fragment(), "/frag");
    }

    #[test]
    fn file_uri_roundtrip() {
        let u = Uri::create_file_uri("/tmp/x.arxml");
        assert!(u.has_authority);
        assert_eq!(u.to_string(), "file:///tmp/x.arxml");
    }

    #[test]
    fn resolve_relative() {
        let base = Uri::parse("file:///a/b/c.y.xmi");
        let rel = Uri::parse("../d.y.xmi");
        let r = rel.resolve(&base);
        // 对齐 artop-cpp 参考算法：`..` 不会折叠回 base 段。
        assert_eq!(r.path(), "/a/b/d.y.xmi");
    }

    #[test]
    fn trim_segments_drops_tail() {
        let u = Uri::parse("/a/b/c.xmi");
        let t = u.trim_segments(1);
        assert_eq!(t.path(), "/a/b");
    }

    #[test]
    fn default_uri_empty() {
        let u = Uri::default();
        assert_eq!(u.to_string(), "");
        assert!(u.is_empty());
        assert!(!u.is_file());
        assert!(!u.is_platform());
    }
}
