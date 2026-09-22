//! Minimal XML escaping helpers (port of the string-escape portion of
//! `emf::xmi::XMLHelper`, C++ `emf-xmi/XMLHelper`).

/// Escape a string for use as an XML attribute value. Escapes `&`, `<`,
/// `>`, `"` and control characters.
pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ if (c as u32) < 0x20 && c != '\n' && c != '\t' && c != '\r' => {
                out.push_str(&format!("&#x{:X};", c as u32))
            }
            _ => out.push(c),
        }
    }
    out
}

/// Escape a string for use as element (mixed) text, escaping `&`, `<`, `>`.
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_attr_special_chars() {
        assert_eq!(
            escape_attr("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f"
        );
    }

    #[test]
    fn escapes_text_only_amp_lt_gt() {
        assert_eq!(escape_text("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d\"e");
    }

    #[test]
    fn roundtrip_attr() {
        let s = escape_attr("x = \"y\" & <z>");
        assert!(
            s.contains("&quot;") && s.contains("&amp;") && s.contains("&lt;") && s.contains("&gt;")
        );
    }
}
