//! Minimal, dependency-free XML DOM for the ARXML layer.
//!
//! Port of the C++ `AutosarXMLLoader`'s pugixml usage (`XmlParser`, `getNodeLocal`,
//! `getNodeText`). Unlike the generic [`emf_xmi`] parser — which keeps only
//! element/text content for the XMI path — this DOM additionally preserves
//! **comments** and **interleaved text (mixed content)**, because the arxml
//! serializer reproduces the document layout (indentation, comments) verbatim.
//!
//! Namespace handling is left to the caller: every element keeps its raw qname
//! and prefix (mirroring pugixml's non-namespace-aware node names).

/// A node inside an element's content, in document order.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A child element.
    Element(Element),
    /// Character data (pcdata or CDATA), preserved with whitespace.
    Text(String),
    /// An XML comment (`<!-- ... -->`); the stored text excludes the delimiters.
    Comment(String),
}

/// An XML element with its raw name, attributes and ordered content.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Raw qualified name, e.g. `"xsi:schemaLocation"`'s owner `"AUTOSAR"`.
    pub name: String,
    /// Namespace prefix (`"ns"`), or `None` when the name is unqualified.
    pub prefix: Option<String>,
    /// Local part (`"AUTOSAR"`).
    pub local: String,
    /// Attributes as `(raw_name, decoded_value)`, in document order.
    pub attrs: Vec<(String, String)>,
    /// Ordered content (elements, text, comments).
    pub children: Vec<Node>,
}

impl Element {
    /// Look up an attribute by its raw qname.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Iterate the child elements (ignoring text and comments).
    pub fn element_children(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// The accumulated character data directly inside this element
    /// (pcdata + CDATA, in order), mirroring the C++ `getNodeText`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for c in &self.children {
            if let Node::Text(t) = c {
                out.push_str(t);
            }
        }
        out
    }

    /// Trimmed accumulated character data.
    pub fn trimmed_text(&self) -> String {
        self.text().trim().to_string()
    }
}

/// Parse a full XML document and return its root element.
///
/// Comments are preserved as [`Node::Comment`], character data (including
/// whitespace) as [`Node::Text`]. The prolog (`<?xml ...?>`), processing
/// instructions and DOCTYPE are skipped.
pub fn parse(src: &str) -> Result<Element, String> {
    let mut p = Parser {
        s: src.as_bytes(),
        pos: 0,
    };
    p.skip_prologue();
    match p.parse_element()? {
        Some(e) => Ok(e),
        None => Err("arxml: no root element".to_string()),
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn starts_with(&self, lit: &str) -> bool {
        self.s[self.pos..].starts_with(lit.as_bytes())
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    /// Skip the prolog: `<?...?>`, `<!--...-->`, `<!DOCTYPE ...>` and whitespace.
    fn skip_prologue(&mut self) {
        loop {
            self.skip_ws();
            if self.starts_with("<?") {
                if let Some(end) = find(self.s, self.pos, b"?>") {
                    self.pos = end + 2;
                } else {
                    self.pos = self.s.len();
                }
            } else if self.starts_with("<!--") {
                if let Some(end) = find(self.s, self.pos, b"-->") {
                    self.pos = end + 3;
                } else {
                    self.pos = self.s.len();
                }
            } else if self.starts_with("<!DOCTYPE") {
                // Skip to the closing '>' of the doctype (ignoring internal subsets).
                if let Some(end) = find(self.s, self.pos, b">") {
                    self.pos = end + 1;
                } else {
                    self.pos = self.s.len();
                }
            } else {
                break;
            }
        }
    }

    fn parse_element(&mut self) -> Result<Option<Element>, String> {
        if self.peek() != Some(b'<') {
            return Ok(None);
        }
        self.pos += 1; // consume '<'
        let name = self.read_name()?;
        let (prefix, local) = split_name(&name);

        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'/') => {
                    // self-closing
                    self.pos += 1;
                    self.expect(b'>')?;
                    return Ok(Some(Element {
                        name,
                        prefix,
                        local,
                        attrs,
                        children: Vec::new(),
                    }));
                }
                Some(b'>') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => {
                    let aname = self.read_name()?;
                    self.skip_ws();
                    self.expect(b'=')?;
                    self.skip_ws();
                    let aval = self.read_quoted()?;
                    attrs.push((aname, aval));
                }
                None => return Err("arxml: unexpected end of input inside tag".to_string()),
            }
        }

        let mut children = Vec::new();
        loop {
            if self.pos >= self.s.len() {
                return Err(format!("arxml: unclosed element <{name}>"));
            }
            if self.starts_with("</") {
                self.pos += 2;
                let cname = self.read_name()?;
                self.skip_ws();
                self.expect(b'>')?;
                if cname != name {
                    return Err(format!(
                        "arxml: mismatched close tag </{cname}> for <{name}>"
                    ));
                }
                return Ok(Some(Element {
                    name,
                    prefix,
                    local,
                    attrs,
                    children,
                }));
            } else if self.starts_with("<!--") {
                if let Some(end) = find(self.s, self.pos, b"-->") {
                    let text = String::from_utf8_lossy(&self.s[self.pos + 4..end]).into_owned();
                    children.push(Node::Comment(text));
                    self.pos = end + 3;
                } else {
                    return Err("arxml: unterminated comment".to_string());
                }
            } else if self.starts_with("<![CDATA[") {
                if let Some(end) = find(self.s, self.pos, b"]]>") {
                    let text = String::from_utf8_lossy(&self.s[self.pos + 9..end]).into_owned();
                    children.push(Node::Text(text));
                    self.pos = end + 3;
                } else {
                    return Err("arxml: unterminated CDATA".to_string());
                }
            } else if self.starts_with("<?") {
                if let Some(end) = find(self.s, self.pos, b"?>") {
                    self.pos = end + 2;
                } else {
                    return Err("arxml: unterminated processing instruction".to_string());
                }
            } else if self.peek() == Some(b'<') {
                if let Some(child) = self.parse_element()? {
                    children.push(Node::Element(child));
                }
            } else {
                let text = self.read_text();
                children.push(Node::Text(text));
            }
        }
    }

    /// Read character data up to the next `<`.
    fn read_text(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b'<' {
                break;
            }
            self.pos += 1;
        }
        decode_entities(&String::from_utf8_lossy(&self.s[start..self.pos]))
    }

    fn read_name(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() || c == b'>' || c == b'/' || c == b'=' {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            return Err("arxml: expected a name".to_string());
        }
        Ok(String::from_utf8_lossy(&self.s[start..self.pos]).into_owned())
    }

    fn read_quoted(&mut self) -> Result<String, String> {
        let quote = match self.peek() {
            Some(q @ (b'"' | b'\'')) => q,
            _ => return Err("arxml: expected a quoted attribute value".to_string()),
        };
        self.pos += 1; // consume opening quote
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == quote {
                let raw = String::from_utf8_lossy(&self.s[start..self.pos]).into_owned();
                self.pos += 1; // consume closing quote
                return Ok(decode_entities(&raw));
            }
            self.pos += 1;
        }
        Err("arxml: unterminated attribute value".to_string())
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("arxml: expected '{}'", c as char))
        }
    }
}

/// Split a raw qname into `(prefix, local)`.
fn split_name(name: &str) -> (Option<String>, String) {
    match name.split_once(':') {
        Some((p, l)) => (Some(p.to_string()), l.to_string()),
        None => (None, name.to_string()),
    }
}

/// Find the first occurrence of `needle` at or after `from`.
fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Decode the 5 predefined XML entities plus numeric character references.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(semi) = bytes[i..].iter().position(|&c| c == b';') {
                let ent = &s[i + 1..i + semi];
                let decoded = match ent {
                    "lt" => Some('<'.to_string()),
                    "gt" => Some('>'.to_string()),
                    "amp" => Some('&'.to_string()),
                    "quot" => Some('"'.to_string()),
                    "apos" => Some('\''.to_string()),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(|c| c.to_string())
                    }
                    _ if ent.starts_with('#') => ent[1..]
                        .parse::<u32>()
                        .ok()
                        .and_then(char::from_u32)
                        .map(|c| c.to_string()),
                    _ => None,
                };
                if let Some(d) = decoded {
                    out.push_str(&d);
                    i += semi + 1;
                    continue;
                }
            }
        }
        // Push the raw byte (safe: XML text is UTF-8 and '&' boundaries are ASCII).
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root_attributes_and_children() {
        let root = parse("<AUTOSAR xmlns=\"urn:x\"><SHORT-NAME>a</SHORT-NAME></AUTOSAR>").unwrap();
        assert_eq!(root.local, "AUTOSAR");
        assert_eq!(root.attr("xmlns"), Some("urn:x"));
        let sn = root.element_children().next().unwrap();
        assert_eq!(sn.local, "SHORT-NAME");
        assert_eq!(sn.text(), "a");
    }

    #[test]
    fn preserves_comments_and_mixed_text() {
        let root = parse("<A>\n  <!-- c -->\n  <B/>\n</A>").unwrap();
        // The comment splits the surrounding whitespace, so the content is
        // text / comment / text / element / text — all five preserved.
        assert_eq!(root.children.len(), 5);
        assert!(matches!(&root.children[0], Node::Text(t) if t == "\n  "));
        assert!(matches!(&root.children[1], Node::Comment(c) if c == " c "));
        assert!(matches!(&root.children[2], Node::Text(t) if t == "\n  "));
        assert!(matches!(&root.children[3], Node::Element(_)));
        assert!(matches!(&root.children[4], Node::Text(t) if t == "\n"));
    }

    #[test]
    fn decodes_entities_and_cdata() {
        let root = parse("<A x=\"a&amp;b\"><![CDATA[<raw>]]></A>").unwrap();
        assert_eq!(root.attr("x"), Some("a&b"));
        assert_eq!(root.text(), "<raw>");
    }

    #[test]
    fn handles_prefixes_and_self_closing() {
        let root = parse("<a:B xmlns:c=\"urn:c\" c:d=\"1\"/>").unwrap();
        assert_eq!(root.prefix.as_deref(), Some("a"));
        assert_eq!(root.local, "B");
        assert_eq!(root.attr("c:d"), Some("1"));
    }
}
