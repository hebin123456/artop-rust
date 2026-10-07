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
///
/// Every variant is boxed/sliced so that `Node` stays 24 bytes: an arxml parse
/// tree holds one `Node` per element *plus* one per text run, and the
/// document-order `Vec<Node>` slot is the single largest consumer of load-time
/// memory. With `Element` stored inline, `Node` was 80 bytes and a 1.3M-element
/// / 2.6M-text-run document spent ~314 MB on slots alone (plus doubling slack);
/// boxing `Element` and using `Box<str>` for character data shrinks the slot to
/// 24 bytes. `Element` is ~50x rarer than text, so the extra indirection per
/// element is far cheaper than the inline layout it replaces.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A child element.
    Element(Box<Element>),
    /// Character data (pcdata or CDATA), preserved with whitespace.
    Text(Box<str>),
    /// An XML comment (`<!-- ... -->`); the stored text excludes the delimiters.
    Comment(Box<str>),
}

/// An XML element with its raw name, attributes and ordered content.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Raw qualified name, e.g. `"xsi:schemaLocation"`'s owner `"AUTOSAR"`.
    pub name: String,
    /// Attributes as `(raw_name, decoded_value)`, in document order.
    pub attrs: Vec<(String, String)>,
    /// Ordered content (elements, text, comments).
    pub children: Vec<Node>,
}

impl Element {
    /// Local part of the name (`"AUTOSAR"`, or `"type"` for `"xsi:type"`).
    ///
    /// Derived from [`Self::name`] rather than stored: for the common
    /// unprefixed arxml element the local part *is* the whole name, so keeping
    /// both fields duplicated every element's name. `Element` is embedded
    /// by value in [`Node`], so shedding its two name strings shrinks both the
    /// element and every `Vec<Node>` slot — the parse tree dominates peak
    /// memory during load.
    pub fn local(&self) -> &str {
        match self.name.split_once(':') {
            Some((_, l)) => l,
            None => &self.name,
        }
    }

    /// Namespace prefix (`"ns"`), or `None` when the name is unqualified.
    pub fn prefix(&self) -> Option<&str> {
        self.name.split_once(':').map(|(p, _)| p)
    }

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
            Node::Element(e) => Some(&**e),
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
                    attrs,
                    children,
                }));
            } else if self.starts_with("<!--") {
                if let Some(end) = find(self.s, self.pos, b"-->") {
                    let text = String::from_utf8_lossy(&self.s[self.pos + 4..end]).into_owned();
                    children.push(Node::Comment(text.into()));
                    self.pos = end + 3;
                } else {
                    return Err("arxml: unterminated comment".to_string());
                }
            } else if self.starts_with("<![CDATA[") {
                if let Some(end) = find(self.s, self.pos, b"]]>") {
                    let text = String::from_utf8_lossy(&self.s[self.pos + 9..end]).into_owned();
                    children.push(Node::Text(text.into()));
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
                    children.push(Node::Element(Box::new(child)));
                }
            } else {
                let text = self.read_text();
                children.push(Node::Text(text.into()));
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

/// Decode one entity body (the text between `&` and `;`) to its character.
fn decode_entity(ent: &str) -> Option<char> {
    Some(match ent {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let code = if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                ent.strip_prefix('#')?.parse::<u32>().ok()?
            };
            char::from_u32(code)?
        }
    })
}

/// Decode the 5 predefined XML entities plus numeric character references.
///
/// Unknown entities (e.g. an undeclared `&foo;`) are left verbatim, exactly like
/// the previous byte-at-a-time version. Decoded runs are copied as whole `&str`
/// slices rather than byte-by-byte: the old loop pushed each raw byte as a
/// `char`, which silently mangled any multi-byte UTF-8 character that shared a
/// text node with an entity (e.g. `é&amp;x` decoded to `Ã©&x`). Copying the
/// untouched spans keeps them valid UTF-8 and is also faster for entity-heavy
/// content, since a run of plain text costs one `push_str` instead of one push
/// per byte.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut last = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(semi) = bytes[i + 1..].iter().position(|&c| c == b';') {
                let semi = i + 1 + semi;
                if let Some(c) = decode_entity(&s[i + 1..semi]) {
                    out.push_str(&s[last..i]);
                    out.push(c);
                    i = semi + 1;
                    last = i;
                    continue;
                }
            }
        }
        i += 1;
    }
    out.push_str(&s[last..]);
    out
}

// ---------------------------------------------------------------------------
// Streaming event reader
// ---------------------------------------------------------------------------

/// A start tag as it appears in the document, before any child content is read.
///
/// The event reader hands this out as soon as the opening tag has been
/// consumed, so the arxml builder can decide how to treat the element (feature
/// lookup, `xsi:type`, `DEST`, …) and *then* pull the events that follow,
/// without ever materialising a whole parse tree.
#[derive(Debug, Clone, PartialEq)]
pub struct StartTag {
    /// Raw qualified name (e.g. `AUTOSAR`).
    pub name: String,
    /// Attributes as `(raw_name, decoded_value)`, in document order.
    pub attrs: Vec<(String, String)>,
    /// `true` for `<TAG/>`: the element has no content and no matching end tag.
    pub self_closing: bool,
}

impl StartTag {
    /// Local part of the name (`"type"` for `"xsi:type"`).
    pub fn local(&self) -> &str {
        match self.name.split_once(':') {
            Some((_, l)) => l,
            None => &self.name,
        }
    }

    /// Look up an attribute by its raw qname.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// One XML event (the streaming counterpart of [`Node`] plus tags).
#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    /// An opening tag (self-closing tags yield no following [`Ev::End`]).
    Start(StartTag),
    /// A closing tag.
    End,
    /// Character data (pcdata or CDATA), preserved with whitespace.
    Text(String),
    /// An XML comment; the text excludes the delimiters.
    Comment(String),
}

/// A dependency-free, allocation-light XML pull reader.
///
/// Unlike [`parse`], which materialises the entire document as a [`Node`] tree,
/// the reader walks the source once and yields events on demand. The arxml
/// loader consumes those events into [`DynamicEObject`](emf_ecore::dynamic)
/// instances directly, so the document tree never has to exist in full — the
/// single largest consumer of load-time memory on big arxml files.
pub struct XmlReader<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> XmlReader<'a> {
    /// A reader positioned at the document's root element (prolog skipped).
    pub fn new(src: &'a str) -> Self {
        let mut p = Parser {
            s: src.as_bytes(),
            pos: 0,
        };
        p.skip_prologue();
        Self { s: p.s, pos: p.pos }
    }

    fn starts_with(&self, lit: &str) -> bool {
        self.s[self.pos..].starts_with(lit.as_bytes())
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    /// The next event, or `None` at end of input.
    pub fn next_event(&mut self) -> Result<Option<Ev>, String> {
        loop {
            if self.pos >= self.s.len() {
                return Ok(None);
            }
            if self.starts_with("<!--") {
                let end = find(self.s, self.pos, b"-->")
                    .ok_or_else(|| "arxml: unterminated comment".to_string())?;
                let text = String::from_utf8_lossy(&self.s[self.pos + 4..end]).into_owned();
                self.pos = end + 3;
                return Ok(Some(Ev::Comment(text)));
            }
            if self.starts_with("<![CDATA[") {
                let end = find(self.s, self.pos, b"]]>")
                    .ok_or_else(|| "arxml: unterminated CDATA".to_string())?;
                let text = String::from_utf8_lossy(&self.s[self.pos + 9..end]).into_owned();
                self.pos = end + 3;
                return Ok(Some(Ev::Text(text)));
            }
            if self.starts_with("<?") {
                // Processing instruction: not part of the model.
                let end = find(self.s, self.pos, b"?>")
                    .ok_or_else(|| "arxml: unterminated processing instruction".to_string())?;
                self.pos = end + 2;
                continue;
            }
            if self.starts_with("</") {
                self.pos += 2;
                let _name = read_name_bytes(self.s, &mut self.pos)?;
                skip_ws_bytes(self.s, &mut self.pos);
                expect_byte(self.s, &mut self.pos, b'>')?;
                return Ok(Some(Ev::End));
            }
            if self.peek() == Some(b'<') {
                return Ok(Some(Ev::Start(self.read_start_tag()?)));
            }
            // Character data up to the next '<'.
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c == b'<' {
                    break;
                }
                self.pos += 1;
            }
            let raw = String::from_utf8_lossy(&self.s[start..self.pos]);
            return Ok(Some(Ev::Text(decode_entities(&raw))));
        }
    }

    fn read_start_tag(&mut self) -> Result<StartTag, String> {
        self.pos += 1; // consume '<'
        let name = read_name_bytes(self.s, &mut self.pos)?;
        let mut attrs = Vec::new();
        loop {
            skip_ws_bytes(self.s, &mut self.pos);
            match self.peek() {
                Some(b'/') => {
                    self.pos += 1;
                    expect_byte(self.s, &mut self.pos, b'>')?;
                    return Ok(StartTag {
                        name,
                        attrs,
                        self_closing: true,
                    });
                }
                Some(b'>') => {
                    self.pos += 1;
                    return Ok(StartTag {
                        name,
                        attrs,
                        self_closing: false,
                    });
                }
                Some(_) => {
                    let aname = read_name_bytes(self.s, &mut self.pos)?;
                    skip_ws_bytes(self.s, &mut self.pos);
                    expect_byte(self.s, &mut self.pos, b'=')?;
                    skip_ws_bytes(self.s, &mut self.pos);
                    let aval = read_quoted_bytes(self.s, &mut self.pos)?;
                    attrs.push((aname, aval));
                }
                None => return Err("arxml: unexpected end of input inside tag".to_string()),
            }
        }
    }
}

/// Read a name into an owned `String`.
fn read_name_bytes(s: &[u8], pos: &mut usize) -> Result<String, String> {
    let start = *pos;
    while let Some(c) = s.get(*pos).copied() {
        if c.is_ascii_whitespace() || c == b'>' || c == b'/' || c == b'=' {
            break;
        }
        *pos += 1;
    }
    if *pos == start {
        return Err("arxml: expected a name".to_string());
    }
    Ok(String::from_utf8_lossy(&s[start..*pos]).into_owned())
}

fn read_quoted_bytes(s: &[u8], pos: &mut usize) -> Result<String, String> {
    let quote = match s.get(*pos).copied() {
        Some(q @ (b'"' | b'\'')) => q,
        _ => return Err("arxml: expected a quoted attribute value".to_string()),
    };
    *pos += 1;
    let start = *pos;
    while let Some(c) = s.get(*pos).copied() {
        if c == quote {
            let raw = String::from_utf8_lossy(&s[start..*pos]);
            *pos += 1;
            return Ok(decode_entities(&raw));
        }
        *pos += 1;
    }
    Err("arxml: unterminated attribute value".to_string())
}

fn skip_ws_bytes(s: &[u8], pos: &mut usize) {
    while matches!(s.get(*pos), Some(c) if c.is_ascii_whitespace()) {
        *pos += 1;
    }
}

fn expect_byte(s: &[u8], pos: &mut usize, c: u8) -> Result<(), String> {
    if s.get(*pos) == Some(&c) {
        *pos += 1;
        Ok(())
    } else {
        Err(format!("arxml: expected '{}'", c as char))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root_attributes_and_children() {
        let root = parse("<AUTOSAR xmlns=\"urn:x\"><SHORT-NAME>a</SHORT-NAME></AUTOSAR>").unwrap();
        assert_eq!(root.local(), "AUTOSAR");
        assert_eq!(root.attr("xmlns"), Some("urn:x"));
        let sn = root.element_children().next().unwrap();
        assert_eq!(sn.local(), "SHORT-NAME");
        assert_eq!(sn.text(), "a");
    }

    #[test]
    fn preserves_comments_and_mixed_text() {
        let root = parse("<A>\n  <!-- c -->\n  <B/>\n</A>").unwrap();
        // The comment splits the surrounding whitespace, so the content is
        // text / comment / text / element / text — all five preserved.
        assert_eq!(root.children.len(), 5);
        assert!(matches!(&root.children[0], Node::Text(t) if &**t == "\n  "));
        assert!(matches!(&root.children[1], Node::Comment(c) if &**c == " c "));
        assert!(matches!(&root.children[2], Node::Text(t) if &**t == "\n  "));
        assert!(matches!(&root.children[3], Node::Element(_)));
        assert!(matches!(&root.children[4], Node::Text(t) if &**t == "\n"));
    }

    #[test]
    fn decodes_entities_and_cdata() {
        let root = parse("<A x=\"a&amp;b\"><![CDATA[<raw>]]></A>").unwrap();
        assert_eq!(root.attr("x"), Some("a&b"));
        assert_eq!(root.text(), "<raw>");
    }

    #[test]
    fn entity_decode_preserves_multibyte_utf8() {
        // A multi-byte character sharing a node with an entity must survive
        // (the old byte-at-a-time decoder turned each byte into a `char`).
        let root = parse("<A>caf\u{e9}&amp;bar</A>").unwrap();
        assert_eq!(root.text(), "caf\u{e9}&bar");
        let root = parse("<A x=\"\u{4e2d}&#x6587;\"/>").unwrap();
        assert_eq!(root.attr("x"), Some("\u{4e2d}\u{6587}"));
        // Unknown entities are preserved verbatim.
        let root = parse("<A>a&amp;b&unknown;c</A>").unwrap();
        assert_eq!(root.text(), "a&b&unknown;c");
    }

    #[test]
    fn handles_prefixes_and_self_closing() {
        let root = parse("<a:B xmlns:c=\"urn:c\" c:d=\"1\"/>").unwrap();
        assert_eq!(root.prefix(), Some("a"));
        assert_eq!(root.local(), "B");
        assert_eq!(root.attr("c:d"), Some("1"));
    }
}
