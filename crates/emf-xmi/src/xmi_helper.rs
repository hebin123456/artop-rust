//! `XMIHelper` free utility functions and namespace constants.
//!
//! Port target: C++ `emf-xmi/XMIHelper.h` inline helpers (`splitQName`,
//! `splitHref`, `stripFragmentSlash`, `escapeXmlAttr`, `escapeXmlText`) and
//! the four namespace URI constants, all aligned to Java
//! `org.eclipse.emf.ecore.xmi.impl.XMIHelperImpl` / `XMLSaveImpl.Escape`.

/// `http://www.eclipse.org/emf/2002/Ecore` (EMF `kEcoreNsURI`).
pub const K_ECORE_NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore";
/// `http://www.omg.org/XMI` (EMF `kXmiNsURI`, the classic XMI 1.x/2.0 URI).
pub const K_XMI_NS_URI: &str = "http://www.omg.org/XMI";
/// `http://schema.omg.org/spec/XMI/2.0` (EMF `kXmiNsURI2`, the XMI 2.1+ URI).
pub const K_XMI_NS_URI_2: &str = "http://schema.omg.org/spec/XMI/2.0";
/// `http://www.w3.org/2001/XMLSchema-instance` (EMF `kXsiNsURI`).
pub const K_XSI_NS_URI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Split a qualified name `"prefix:local"` into `(prefix, local)`.
///
/// With no colon the prefix is empty and the whole string is the local part;
/// an empty input yields two empty parts. EMF `XMIHelper.splitQName`.
pub fn split_qname(qname: &str) -> (String, String) {
    match qname.find(':') {
        Some(i) => (qname[..i].to_string(), qname[i + 1..].to_string()),
        None => (String::new(), qname.to_string()),
    }
}

/// The two components of an href URI: the (external) resource path and the
/// content fragment. EMF `XMIHelper.splitHref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HrefParts {
    /// Everything before the fragment, e.g. `library.ecore` or the raw
    /// namespace URI when the two-part `"ecore:EDataType <uri>"` form is used.
    pub path: String,
    /// Everything after the `#` (fragment, without the `#`).
    pub fragment: String,
}

/// Split an href string into `{path, fragment}`.
///
/// A leading `"prefix:Type "<space>` wrapper (the two-part forms such as
/// `"ecore:EDataType http://.../Ecore#//EString"`) is stripped before the
/// fragment split, so `path` holds the resource/URI and `fragment` the
/// `//Class`-style tail. EMF `XMIHelper.splitHref`.
pub fn split_href(href: &str) -> HrefParts {
    // Drop an optional `prefix:Type ` prefix up through the first space.
    let core = match href.find(' ') {
        Some(i) => &href[i + 1..],
        None => href,
    };
    match core.find('#') {
        Some(i) => HrefParts {
            path: core[..i].to_string(),
            fragment: core[i + 1..].to_string(),
        },
        None => HrefParts {
            path: core.to_string(),
            fragment: String::new(),
        },
    }
}

/// Remove the leading `/`(s) from a URI fragment so `//Container/x` becomes
/// `Container/x`. EMF `XMIHelper.stripFragmentSlash`.
pub fn strip_fragment_slash(fragment: &str) -> String {
    fragment.trim_start_matches('/').to_string()
}

/// Default mappable-limit: characters above ASCII are emitted as numeric
/// character references (matches the test "ASCII encoding, mappableLimit=0x7F").
const DEFAULT_MAPPABLE_LIMIT: u32 = 0x7F;

/// A numeric character reference with lower-case hex (`&#x4e2d;`), as EMF's
/// `convert`/`convertText` emit.
#[inline]
fn numeric_ref(cp: u32) -> String {
    format!("&#x{:x};", cp)
}

/// Escape a string as an XML **attribute** value.
///
/// Escapes `& < "` and the control whitespace `\n \r \t` (as `&#xA;` `&#xD;`
/// `&#x9;`); leaves `>` and `'` untouched; encodes any code point above the
/// default mappable limit (ASCII, `0x7F`) as a lower-case numeric reference.
/// EMF `XMIHelper.escapeXmlAttr` (Java `XMLSaveImpl.Escape.convert`).
pub fn escape_xml_attr(s: &str) -> String {
    escape_xml_attr_with_limit(s, DEFAULT_MAPPABLE_LIMIT)
}

/// `escape_xml_attr` with an explicit mapping limit (`0x10FFFF` passes all
/// characters through verbatim, i.e. UTF-8 passthrough).
pub fn escape_xml_attr_with_limit(s: &str, mappable_limit: u32) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            '\t' => out.push_str("&#x9;"),
            _ if cp > mappable_limit => out.push_str(&numeric_ref(cp)),
            _ if c.is_control() => out.push_str(&numeric_ref(cp)),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a string as XML **text/mixed** content.
///
/// Escapes `& < "` and `\r` (as `&#xD;`), keeps `> ' \n \t` as-is, and above
/// the default mappable limit encodes non-ASCII as lower-case numeric refs.
/// EMF `XMIHelper.escapeXmlText` (Java `XMLSaveImpl.Escape.convertText`).
pub fn escape_xml_text(s: &str) -> String {
    escape_xml_text_with_limit(s, DEFAULT_MAPPABLE_LIMIT)
}

/// `escape_xml_text` with an explicit mapping limit.
pub fn escape_xml_text_with_limit(s: &str, mappable_limit: u32) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#xD;"),
            c if c == '\n' || c == '\t' => out.push(c),
            _ if cp > mappable_limit => out.push_str(&numeric_ref(cp)),
            _ if c.is_control() => out.push_str(&numeric_ref(cp)),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_qname_cases() {
        assert_eq!(split_qname("ecore:EClass"), ("ecore".into(), "EClass".into()));
        assert_eq!(split_qname("EPackage"), (String::new(), "EPackage".into()));
        assert_eq!(split_qname(""), (String::new(), String::new()));
    }

    #[test]
    fn split_href_cases() {
        let r = split_href("library.ecore#//Library");
        assert_eq!(r.path, "library.ecore");
        assert_eq!(r.fragment, "//Library");

        let r = split_href("#//Book");
        assert_eq!(r.path, "");
        assert_eq!(r.fragment, "//Book");

        let r = split_href("other.ecore");
        assert_eq!(r.path, "other.ecore");
        assert_eq!(r.fragment, "");

        // Two-part "ecore:EDataType <uri>" form: strip to the URI, then split.
        let r = split_href("ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString");
        assert_eq!(r.path, "http://www.eclipse.org/emf/2002/Ecore");
        assert_eq!(r.fragment, "//EString");

        let r = split_href("ecore:EClass library.ecore#//Book");
        assert_eq!(r.path, "library.ecore");
        assert_eq!(r.fragment, "//Book");
    }

    #[test]
    fn strip_fragment_slash_cases() {
        assert_eq!(strip_fragment_slash("//EString"), "EString");
        assert_eq!(strip_fragment_slash("/Library"), "Library");
        assert_eq!(strip_fragment_slash("Book"), "Book");
        assert_eq!(strip_fragment_slash("//Container/feature.name"), "Container/feature.name");
        assert_eq!(strip_fragment_slash(""), "");
    }

    #[test]
    fn escape_attr_basic() {
        assert_eq!(escape_xml_attr("a&b"), "a&amp;b");
        assert_eq!(escape_xml_attr("a<b"), "a&lt;b");
        assert_eq!(escape_xml_attr("a\"b"), "a&quot;b");
        // Java attribute values leave > and ' unescaped.
        assert_eq!(escape_xml_attr("a>b"), "a>b");
        assert_eq!(escape_xml_attr("a'b"), "a'b");
    }

    #[test]
    fn escape_attr_control_whitespace() {
        assert_eq!(escape_xml_attr("a\nb"), "a&#xA;b");
        assert_eq!(escape_xml_attr("a\rb"), "a&#xD;b");
        assert_eq!(escape_xml_attr("a\tb"), "a&#x9;b");
        assert_eq!(escape_xml_attr("hello world 123"), "hello world 123");
        assert_eq!(escape_xml_attr("<&>\"'"), "&lt;&amp;>&quot;'");
    }

    #[test]
    fn escape_attr_non_ascii_and_passthrough() {
        // 中
        assert_eq!(escape_xml_attr("a中b"), "a&#x4e2d;b");
        // é
        assert_eq!(escape_xml_attr("aéb"), "a&#xe9;b");
        // UTF-8 passthrough
        assert_eq!(escape_xml_attr_with_limit("a中b", 0x10FFFF), "a中b");
        assert_eq!(escape_xml_attr(""), "");
    }

    #[test]
    fn escape_text_cases() {
        assert_eq!(escape_xml_text("a&b"), "a&amp;b");
        assert_eq!(escape_xml_text("a<b"), "a&lt;b");
        assert_eq!(escape_xml_text("a\"b"), "a&quot;b");
        assert_eq!(escape_xml_text("a>b"), "a>b");
        assert_eq!(escape_xml_text("a'b"), "a'b");
        // convertText keeps \n and \t but escapes \r.
        assert_eq!(escape_xml_text("a\nb"), "a\nb");
        assert_eq!(escape_xml_text("a\tb"), "a\tb");
        assert_eq!(escape_xml_text("a\rb"), "a&#xD;b");
        assert_eq!(escape_xml_text("plain text 42"), "plain text 42");
    }

    #[test]
    fn namespace_constants_match() {
        assert_eq!(K_ECORE_NS_URI, "http://www.eclipse.org/emf/2002/Ecore");
        assert_eq!(K_XMI_NS_URI, "http://www.omg.org/XMI");
        assert_eq!(K_XMI_NS_URI_2, "http://schema.omg.org/spec/XMI/2.0");
        assert_eq!(K_XSI_NS_URI, "http://www.w3.org/2001/XMLSchema-instance");
    }
}