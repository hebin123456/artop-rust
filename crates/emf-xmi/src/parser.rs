//! Minimal, dependency-free XML element-tree parser for the `emf-xmi` loader.
//!
//! Handles elements, attributes, text, self-closing tags, comments, PIs and
//! CDATA; decodes the 5 predefined entities plus numeric character references.
//! Namespace-awareness is intentionally left to the caller: each element keeps
//! its raw qname and prefix so the loader can resolve `xmlns` declarations
//! against its own registry.

#[derive(Debug, Clone, PartialEq)]
pub struct XmlNode {
    /// Raw qualified name, e.g. `"lib:Book"`.
    pub name: String,
    /// Namespace prefix (`"lib"`), or `None` when the name is unqualified.
    pub prefix: Option<String>,
    /// Local part (`"Book"`).
    pub local: String,
    /// Attributes as `(raw_name, decoded_value)`.
    pub attrs: Vec<(String, String)>,
    /// Child elements.
    pub children: Vec<XmlNode>,
    /// Concatenated non-whitespace text within the element (between children).
    pub text: String,
}

impl XmlNode {
    /// Look up an attribute by its raw qname.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
    /// First child element matching `local` (bare) or full qname.
    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }
}

/// Parse a full XML document; returns the (non-PI, non-comment) top-level
/// elements.
pub fn parse(src: &str) -> Result<Vec<XmlNode>, String> {
    let mut p = Parser { s: src, pos: 0 };
    p.parse_document()
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn parse_document(&mut self) -> Result<Vec<XmlNode>, String> {
        let mut roots = Vec::new();
        loop {
            self.skip_prologue();
            if self.pos >= self.s.len() {
                break;
            }
            match self.peek() {
                Some('<') => {
                    if self.rest().starts_with("</") {
                        // stray closing tag; tolerate and stop
                        break;
                    }
                    let node = self.parse_element()?;
                    roots.push(node);
                }
                Some(_) => {
                    // stray text between elements; skip
                    self.skip_raw_text();
                }
                None => break,
            }
        }
        Ok(roots)
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn skip_prologue(&mut self) {
        loop {
            let r = self.rest();
            // Comments
            if r.starts_with("<!--") {
                if let Some(end) = r.find("-->") {
                    self.pos += end + 3;
                    continue;
                }
            }
            // PI / XML declaration
            if r.starts_with("<?") {
                if let Some(end) = r.find("?>") {
                    self.pos += end + 2;
                    continue;
                }
            }
            // DOCTYPE
            if r.starts_with("<!DOCTYPE") || r.starts_with("<!doctype") {
                if let Some(end) = r.find('>') {
                    self.pos += end + 1;
                    continue;
                }
            }
            // Whitespace
            let trimmed = r.trim_start();
            let skip = r.len() - trimmed.len();
            if skip > 0 {
                self.pos += skip;
                continue;
            }
            break;
        }
    }

    fn parse_element(&mut self) -> Result<XmlNode, String> {
        debug_assert!(self.rest().starts_with('<') && !self.rest().starts_with("</"));
        // consume '<' and name
        self.pos += 1;
        let name = self.read_name();
        let (prefix, local) = split_qname(&name);

        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("/>") {
                self.pos += 2;
                return Ok(XmlNode {
                    name,
                    prefix,
                    local,
                    attrs,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            if r.starts_with('>') {
                self.pos += 1;
                break;
            }
            if r.is_empty() || r.starts_with('<') {
                return Err(format!("malformed start tag for <{}>", name));
            }
            // attribute name="value" | name='value'
            let aname = self.read_name();
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(format!("expected '=' after attribute {}", aname));
            }
            self.pos += 1;
            self.skip_ws();
            let quote = match self.peek() {
                Some(q @ ('"' | '\'')) => {
                    self.pos += 1; // consume quote, q has len 1 in bytes
                    q
                }
                _ => return Err(format!("expected quoted value for attribute {}", aname)),
            };
            let value = self.read_until_quote(quote)?;
            attrs.push((aname, decode_entities(&value)));
        }

        // content
        let mut children = Vec::new();
        let mut text = String::new();
        loop {
            let r = self.rest();
            if r.starts_with("</") {
                // closing tag
                self.pos += 2;
                let cname = self.read_name();
                self.skip_ws();
                if !self.rest().starts_with('>') {
                    return Err(format!("malformed closing tag </{}>", cname));
                }
                self.pos += 1;
                if cname != name {
                    return Err(format!(
                        "mismatched closing tag </{}> for <{}>",
                        cname, name
                    ));
                }
                break;
            }
            if r.starts_with("<!--") {
                if let Some(end) = r.find("-->") {
                    self.pos += end + 3;
                    continue;
                }
                return Err("unterminated comment".to_string());
            }
            if r.starts_with("<![CDATA[") {
                if let Some(end) = r.find("]]>") {
                    text.push_str(&r[9..end].to_string());
                    self.pos += end + 3;
                    continue;
                }
                return Err("unterminated CDATA".to_string());
            }
            if r.starts_with("<?") {
                if let Some(end) = r.find("?>") {
                    self.pos += end + 2;
                    continue;
                }
                return Err("unterminated PI".to_string());
            }
            if r.starts_with('<') {
                let node = self.parse_element()?;
                children.push(node);
                continue;
            }
            // raw text until next '<' (decode entities; CDATA is already literal)
            let text_end = r.find('<').unwrap_or(r.len());
            text.push_str(&decode_entities(&r[..text_end]));
            self.pos += text_end;
        }

        let trimmed_text = text.trim().to_string();
        Ok(XmlNode {
            name,
            prefix,
            local,
            attrs,
            children,
            text: trimmed_text,
        })
    }

    fn read_name(&mut self) -> String {
        self.skip_ws();
        let r = self.rest();
        let mut end = 0;
        let mut first = true;
        for c in r.chars() {
            let valid = if first {
                c.is_alphabetic() || c == '_' || c == ':'
            } else {
                c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':'
            };
            if !valid {
                break;
            }
            end += c.len_utf8();
            first = false;
        }
        let name = &r[..end];
        self.pos += end;
        name.to_string()
    }

    fn read_until_quote(&mut self, quote: char) -> Result<String, String> {
        let r = self.rest();
        let mut end = 0;
        for c in r.chars() {
            if c == quote {
                break;
            }
            end += c.len_utf8();
        }
        if end >= r.len() {
            return Err("unterminated attribute value".to_string());
        }
        let val = &r[..end];
        self.pos += end + quote.len_utf8();
        Ok(val.to_string())
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        let trimmed = r.trim_start();
        self.pos += r.len() - trimmed.len();
    }

    fn skip_raw_text(&mut self) {
        let r = self.rest();
        if let Some(idx) = r.find('<') {
            self.pos += idx;
        } else {
            self.pos = self.s.len();
        }
    }
}

fn split_qname(name: &str) -> (Option<String>, String) {
    match name.find(':') {
        Some(idx) => (Some(name[..idx].to_string()), name[idx + 1..].to_string()),
        None => (None, name.to_string()),
    }
}

/// Decode the 5 predefined entities and numeric references in a string.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '&' {
            // find ';'
            let mut j = i + 1;
            let mut code = String::new();
            while j < chars.len() && chars[j] != ';' && chars[j] != '&' {
                code.push(chars[j]);
                j += 1;
            }
            if j < chars.len() && chars[j] == ';' {
                let decoded = match code.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "#10" => Some('\n'),
                    "#13" => Some('\r'),
                    "#9" => Some('\t'),
                    _ => decode_numeric(&code).map(|cp| char::from_u32(cp)).flatten(),
                };
                match decoded {
                    Some(d) => {
                        out.push(d);
                        i = j + 1;
                        continue;
                    }
                    None => {
                        // unknown entity; keep raw
                        out.push('&');
                        i += 1;
                        continue;
                    }
                }
            }
            out.push('&');
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn decode_numeric(code: &str) -> Option<u32> {
    if let Some(hex) = code.strip_prefix("#x").or_else(|| code.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()
    } else if let Some(dec) = code.strip_prefix('#') {
        dec.parse::<u32>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_tree() {
        let nodes = parse("<lib:Book title=\"Hi\"><chapters x=\"1\"/></lib:Book>").unwrap();
        assert_eq!(nodes.len(), 1);
        let n = &nodes[0];
        assert_eq!(n.name, "lib:Book");
        assert_eq!(n.prefix.as_deref(), Some("lib"));
        assert_eq!(n.local, "Book");
        assert_eq!(n.attr("title"), Some("Hi"));
        assert_eq!(n.children.len(), 1);
        assert_eq!(n.children[0].local, "chapters");
    }

    #[test]
    fn parses_xml_declaration_and_comment() {
        let src = "<?xml version=\"1.0\"?><!-- drop --><a/>";
        let nodes = parse(src).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].local, "a");
    }

    #[test]
    fn decodes_entities() {
        let nodes = parse("<a t=\"a&amp;b&lt;c\" >x &gt; y</a>").unwrap();
        assert_eq!(nodes[0].attr("t"), Some("a&b<c"));
        assert_eq!(nodes[0].text, "x > y");
    }

    #[test]
    fn decodes_text_with_children() {
        let nodes = parse("<a>hello<b/>world</a>").unwrap();
        assert_eq!(nodes[0].children.len(), 1);
        // whitespace-trimmed concatenated text
        assert!(nodes[0].text.contains("hello"));
        assert!(nodes[0].text.contains("world"));
    }

    #[test]
    fn errors_on_mismatched_close() {
        assert!(parse("<a></b>").is_err());
    }
}
