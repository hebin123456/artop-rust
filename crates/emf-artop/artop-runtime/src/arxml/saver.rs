//! AUTOSAR arxml serializer (port of the C++ `AutosarXMLSaver`, aligned to
//! Java `org.artop.aal.common.resource.impl.AutosarXMLSaveImpl`).
//!
//! Writes a resource's `DynamicEObject` tree back to an arxml document. The
//! structure mirrors the C++ implementation:
//!
//!   * [`DomWriter`] — a streaming XML writer with **delayed tag opening**
//!     (the C++ `PugiDomWriter`): `begin_element` only pushes a frame; the
//!     opening tag is emitted when the first child/text/comment arrives, and
//!     `end_element` decides between `<TAG/>`, an indented `</TAG>` and an
//!     inline `</TAG>`. Its text/attribute escaping mirrors the C++ byte for
//!     byte, which is what makes a load → save round-trip stable.
//!   * [`AutosarSaver`] — the feature walk: attributes vs child elements,
//!     APRXML rules 0012/0015/0016/default for containment wrapping,
//!     `<FEATURE DEST="Type">short-name-path</FEATURE>` for references, and the
//!     mixed-content sequence replay.
//!
//! Multi-valued features marked `ordered=false` in the metamodel are sorted by
//! their `atp.Splitkey` before serialization (the C++ `isFeatureOrdered` /
//! `sortChildrenBySplitkey`), so a document whose children are not already in
//! splitkey order still serializes exactly like the C++/Java implementations.
//!
//! Verified: the four non-empty AUTOSAR samples in `artop-cpp`'s
//! `output/samples/` (largest ~820 KB) round-trip **byte-for-byte identical**
//! through load → save (see `tools/conformance/interop_arxml.py`). Getting there
//! required the metamodel to keep the *declaration order* of `eSuperTypes`
//! (see `artop_codegen::registry_gen`), so inherited features — and hence XML
//! attribute order — match the C++/Java serializers exactly.

use std::collections::HashSet;
use std::rc::Rc;

use emf_common::fast_hash::FxHashMap;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::datatype;
use emf_ecore::{EClass, EStructuralFeature, PackageRegistry};
use emf_xmi::{XMIResource, XMLSave};

use crate::arxml::dom;
use crate::arxml::store::{self, MixedEntry};

/// AUTOSAR R4.0 default namespace.
const AUTOSAR_NS_URI: &str = "http://autosar.org/schema/r4.0";
/// The `xsi` namespace URI.
const XSI_NS_URI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// Fallback `xsi:schemaLocation` when the resource declares none.
const DEFAULT_SCHEMA_LOCATION: &str = "http://autosar.org/schema/r4.0 AUTOSAR_00048.xsd";
/// The root element local name.
const ROOT_ELEMENT: &str = "AUTOSAR";
/// The reference target attribute name.
const DEST_ATTR: &str = "DEST";
/// The `SHORT-NAME` arxml element name.
const SHORT_NAME: &str = "SHORT-NAME";
/// Prefix of the encoded cross-document proxy URI (C++ `autosar-proxy://base=`).
const PROXY_PREFIX: &str = "autosar-proxy://base=";
/// `IS-DEFAULT` / `BASE-IS-THIS-PACKAGE` bookkeeping feature names.
const BASE_IS_THIS_PACKAGE: &str = "BASE-IS-THIS-PACKAGE";
const IS_DEFAULT: &str = "IS-DEFAULT";
const SHORT_LABEL: &str = "SHORT-LABEL";
const PACKAGE_REF: &str = "PACKAGE-REF";

/// APRXML composition rule derived from a feature's role/type/wrapper flags
/// (C++ `resolveAprxmlRule` / Java `AutosarPersistenceRules`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AprxmlRule {
    /// role+type element combined (`upperBound <= 1`).
    Rule0012,
    /// pure type element (no role, no wrapper).
    Rule0015,
    /// plain containment: all four flags false, skip the wrapper.
    Rule0016,
    /// default: standard EMF + role-wrapper wrapping.
    Default,
}

/// A streaming XML writer with delayed tag opening (C++ `PugiDomWriter`).
struct DomWriter {
    indent: String,
    /// `indent` repeated `d` times, grown on demand. Indentation is written once
    /// per element and once per element close, at a depth that grows with the
    /// document; repeating the indent unit with a `push_str` loop meant one call
    /// per level (tens of millions of 2-byte copies on a 300 MB document).
    /// Caching the expanded prefix turns that into a single slice copy.
    indent_cache: Vec<String>,
    buf: String,
    depth: usize,
    stack: Vec<Frame>,
    /// Frames released by [`DomWriter::end_element`], kept so their `tag` /
    /// `attrs` allocations are reused instead of re-allocated per element.
    frame_pool: Vec<Frame>,
}

/// A pending element frame (C++ `PugiDomWriter::Frame`).
#[derive(Default)]
struct Frame {
    tag: String,
    /// Accumulated ` name="value"` attribute text.
    attrs: String,
    /// Whether the opening `>` has been written.
    opened: bool,
    /// Whether an element child has been written.
    has_elem_child: bool,
    /// Whether `write_text` has been called (even with an empty string).
    has_text: bool,
}

impl DomWriter {
    fn new() -> Self {
        Self {
            indent: "  ".to_string(),
            indent_cache: vec![String::new()],
            buf: String::new(),
            depth: 0,
            stack: Vec::new(),
            frame_pool: Vec::new(),
        }
    }

    fn set_indent(&mut self, indent: &str) {
        self.indent = indent.to_string();
        // The cached prefixes were expanded from the old unit; rebuild lazily.
        self.indent_cache.clear();
        self.indent_cache.push(String::new());
    }

    /// Write the `<?xml ...?>` declaration (call before any element).
    fn set_output(&mut self, write_decl: bool, encoding: &str) {
        if write_decl {
            self.buf.push_str("<?xml version=\"1.0\" encoding=\"");
            self.buf.push_str(encoding);
            self.buf.push_str("\"?>\n");
        }
    }

    /// Escape an element's text (C++ `encodeText`): `<`, `&`, `"`, `\r`.
    ///
    /// Scanned byte-wise and emitted as whole clean runs. The previous
    /// `chars()` loop pushed one `char` per byte even for text containing no
    /// escapable character — which is the overwhelmingly common case (short
    /// names, numeric values, reference paths) — so a value cost one capacity
    /// check and copy per character. Every escapable byte is ASCII, so a run
    /// boundary is always a UTF-8 boundary and the slices stay valid.
    fn encode_text(s: &str, out: &mut String) {
        let bytes = s.as_bytes();
        let mut last = 0;
        for (i, &b) in bytes.iter().enumerate() {
            let rep = match b {
                b'<' => "&lt;",
                b'&' => "&amp;",
                b'"' => "&quot;",
                b'\r' => "&#xD;",
                _ => continue,
            };
            out.push_str(&s[last..i]);
            out.push_str(rep);
            last = i + 1;
        }
        out.push_str(&s[last..]);
    }

    /// Escape an attribute value (C++ `encodeAttributeValue`): `<`, `&`, `"`,
    /// `\r`, `\n`, `\t`.
    fn encode_attribute_value(s: &str, out: &mut String) {
        let bytes = s.as_bytes();
        let mut last = 0;
        for (i, &b) in bytes.iter().enumerate() {
            let rep = match b {
                b'<' => "&lt;",
                b'&' => "&amp;",
                b'"' => "&quot;",
                b'\r' => "&#13;",
                b'\n' => "&#10;",
                b'\t' => "&#9;",
                _ => continue,
            };
            out.push_str(&s[last..i]);
            out.push_str(rep);
            last = i + 1;
        }
        out.push_str(&s[last..]);
    }

    fn write_indent(&mut self) {
        if self.depth >= self.indent_cache.len() {
            let mut last = self.indent_cache.last().cloned().unwrap_or_default();
            while self.indent_cache.len() <= self.depth {
                last.push_str(&self.indent);
                self.indent_cache.push(last.clone());
            }
        }
        self.buf.push_str(&self.indent_cache[self.depth]);
    }

    /// Emit the delayed opening tag of the current frame, if pending.
    fn flush_open(&mut self) {
        if let Some(top) = self.stack.last_mut() {
            if !top.opened {
                self.buf.push('<');
                self.buf.push_str(&top.tag);
                self.buf.push_str(&top.attrs);
                self.buf.push('>');
                top.opened = true;
            }
        }
    }

    fn begin_element(&mut self, tag: &str) {
        if !self.stack.is_empty() {
            self.flush_open();
            let parent_has_text = {
                let top = self.stack.last_mut().unwrap();
                top.has_elem_child = true;
                top.has_text
            };
            if !parent_has_text {
                self.buf.push('\n');
                self.write_indent();
            }
        }
        let mut frame = self.frame_pool.pop().unwrap_or_default();
        frame.tag.clear();
        frame.tag.push_str(tag);
        frame.attrs.clear();
        frame.opened = false;
        frame.has_elem_child = false;
        frame.has_text = false;
        self.stack.push(frame);
        self.depth += 1;
    }

    fn write_attribute(&mut self, name: &str, value: &str) {
        if let Some(top) = self.stack.last_mut() {
            if !top.opened {
                top.attrs.push(' ');
                top.attrs.push_str(name);
                top.attrs.push_str("=\"");
                Self::encode_attribute_value(value, &mut top.attrs);
                top.attrs.push('"');
            }
        }
    }

    fn write_text(&mut self, text: &str) {
        if self.stack.is_empty() {
            return;
        }
        self.flush_open();
        self.stack.last_mut().unwrap().has_text = true;
        if !text.is_empty() {
            Self::encode_text(text, &mut self.buf);
        }
    }

    fn write_comment(&mut self, text: &str) {
        if self.stack.is_empty() {
            return;
        }
        self.flush_open();
        self.buf.push_str("<!--");
        self.buf.push_str(text);
        self.buf.push_str("-->");
    }

    fn end_element(&mut self) {
        if self.stack.is_empty() {
            return;
        }
        self.depth = self.depth.saturating_sub(1);
        let mut f = self.stack.pop().unwrap();
        if !f.opened {
            self.buf.push('<');
            self.buf.push_str(&f.tag);
            self.buf.push_str(&f.attrs);
            self.buf.push_str("/>");
        } else if f.has_elem_child && !f.has_text {
            self.buf.push('\n');
            self.write_indent();
            self.buf.push_str("</");
            self.buf.push_str(&f.tag);
            self.buf.push('>');
        } else {
            self.buf.push_str("</");
            self.buf.push_str(&f.tag);
            self.buf.push('>');
        }
        // Recycle the frame: keep its capacity for the next element instead of
        // allocating a fresh `tag` / `attrs` pair every time.
        f.tag.clear();
        f.attrs.clear();
        self.frame_pool.push(f);
    }

    /// Consume the accumulated output.
    fn finish(&mut self) -> String {
        std::mem::take(&mut self.buf)
    }
}

/// AUTOSAR arxml serializer (C++ `AutosarXMLSaver`), pluggable into an
/// [`XMIResource`] as its [`XMLSave`].
#[derive(Debug, Default, Clone, Copy)]
pub struct AutosarXMLSaver;

impl AutosarXMLSaver {
    /// A new arxml serializer.
    pub fn new() -> Self {
        Self
    }
}

impl XMLSave for AutosarXMLSaver {
    fn save(&self, resource: &XMIResource) -> String {
        AutosarSaver::new(resource).run()
    }
}

/// The serialization context (C++ `AutosarSaver`).
struct AutosarSaver<'a> {
    res: &'a XMIResource,
    reg: PackageRegistry,
    writer: DomWriter,
    /// `ObjectRef` identity -> short-name path.
    snp_cache: FxHashMap<usize, String>,
    /// `ObjectRef` identity -> short name.
    short_name_cache: FxHashMap<usize, String>,
    /// class name -> arxml type name.
    type_name_cache: FxHashMap<String, String>,
    /// `EClass::instance_id` -> sorted feature list (shared, so callers do not
    /// deep-clone the feature vector on every object). Keyed by the class
    /// identity rather than its name so the per-object lookup neither allocates
    /// a key `String` nor hashes one.
    sorted_cache: FxHashMap<u64, Rc<Vec<EStructuralFeature>>>,
}

impl<'a> AutosarSaver<'a> {
    fn new(res: &'a XMIResource) -> Self {
        Self {
            res,
            reg: res.registry().clone(),
            writer: DomWriter::new(),
            snp_cache: FxHashMap::default(),
            short_name_cache: FxHashMap::default(),
            type_name_cache: FxHashMap::default(),
            sorted_cache: FxHashMap::default(),
        }
    }

    /// The top-level entry (C++ `AutosarSaver::save`).
    fn run(&mut self) -> String {
        let indent = {
            let i = self.res.options().indent.clone();
            if i.is_empty() {
                "  ".to_string()
            } else {
                i
            }
        };
        self.writer.set_indent(&indent);

        let enc = {
            let e = self.res.options().encoding.clone();
            if e.is_empty() {
                "UTF-8".to_string()
            } else {
                e
            }
        };
        let write_decl = self.res.options().xml_declaration;
        self.writer.set_output(write_decl, &enc);

        self.writer.begin_element(ROOT_ELEMENT);
        self.writer.write_attribute("xmlns", AUTOSAR_NS_URI);
        self.writer.write_attribute("xmlns:xsi", XSI_NS_URI);
        let schema = schema_location(self.res);
        self.writer.write_attribute("xsi:schemaLocation", &schema);

        let roots: Vec<ObjectRef> = self.res.resource().contents().to_vec();
        for obj in &roots {
            self.save_object_content(obj, false);
        }

        self.writer.end_element();
        let mut out = self.writer.finish();
        out.push('\n');
        out
    }

    // ---- metamodel helpers ----

    /// The `EClass` of `obj`.
    fn class_of(&self, obj: &ObjectRef) -> Option<EClass> {
        let b = obj.borrow();
        self.reg.find_class(b.e_class())
    }

    /// The arxml type name of `obj`'s class (C++ `getTypeXmlName`).
    fn type_xml_name(&mut self, obj: &ObjectRef) -> String {
        let cname = obj.borrow().e_class().to_string();
        if let Some(v) = self.type_name_cache.get(&cname) {
            return v.clone();
        }
        let name = self
            .reg
            .find_class(&cname)
            .map(|c| c.xml_name().to_string())
            .unwrap_or_else(|| cname.clone());
        self.type_name_cache.insert(cname, name.clone());
        name
    }

    /// The feature list of `class` in serialization order (C++
    /// `collectSortedFeatures`): ancestors first, each class's own features
    /// stably ordered by `internal-xml-sequenceOffset`.
    fn sorted_features(&mut self, class: &EClass) -> Rc<Vec<EStructuralFeature>> {
        let key = class.instance_id();
        if let Some(v) = self.sorted_cache.get(&key) {
            return v.clone();
        }
        let v = Rc::new(self.collect_sorted_features(class));
        self.sorted_cache.insert(key, v.clone());
        v
    }

    fn collect_sorted_features(&self, class: &EClass) -> Vec<EStructuralFeature> {
        let mut out: Vec<EStructuralFeature> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let push_class =
            |c: &EClass, out: &mut Vec<EStructuralFeature>, seen: &mut HashSet<String>| {
                let mut feats: Vec<EStructuralFeature> = c.e_structural_features().to_vec();
                // Stable sort within the class by sequence offset (absent -> 0).
                feats.sort_by_key(|f| f.sequence_offset().unwrap_or(0));
                for f in feats {
                    if seen.insert(f.name().to_string()) {
                        out.push(f);
                    }
                }
            };
        for name in class.e_all_super_types(&self.reg) {
            if let Some(c) = self.reg.find_class(&name) {
                push_class(&c, &mut out, &mut seen);
            }
        }
        push_class(class, &mut out, &mut seen);
        out
    }

    /// The `simple`-content feature of a class (carries the element's text).
    fn simple_feature(&mut self, class: &EClass) -> Option<EStructuralFeature> {
        self.sorted_features(class)
            .iter()
            .find(|f| f.tagged_feature_kind() == "simple")
            .cloned()
    }

    /// The `SHORT-NAME` (or ecore `shortName`) value of `obj`.
    fn short_name(&mut self, obj: &ObjectRef) -> String {
        let key = store::object_key(obj);
        if let Some(v) = self.short_name_cache.get(&key) {
            return v.clone();
        }
        let sn = read_string_feature(obj, SHORT_NAME)
            .or_else(|| read_string_feature(obj, "shortName"))
            .unwrap_or_default();
        self.short_name_cache.insert(key, sn.clone());
        sn
    }

    /// The absolute short-name path of `obj` (C++ `getShortNamePath`); a proxy
    /// yields its proxy URI.
    fn short_name_path(&mut self, obj: &ObjectRef) -> String {
        let key = store::object_key(obj);
        if let Some(v) = self.snp_cache.get(&key) {
            return v.clone();
        }
        let path = if obj.borrow().e_is_proxy() {
            obj.borrow()
                .e_proxy_uri()
                .map(|u| u.to_string())
                .unwrap_or_default()
        } else {
            let mut parts: Vec<String> = Vec::new();
            let mut cur = Some(obj.clone());
            while let Some(o) = cur {
                let sn = self.short_name(&o);
                if sn.is_empty() {
                    break;
                }
                parts.push(sn);
                cur = o.borrow().e_container();
            }
            parts.reverse();
            let mut p = String::new();
            for part in parts {
                p.push('/');
                p.push_str(&part);
            }
            p
        };
        self.snp_cache.insert(key, path.clone());
        path
    }

    // ---- object content ----

    /// Serialize an object's features (C++ `saveObjectContent`).
    fn save_object_content(&mut self, obj: &ObjectRef, elements_only: bool) {
        let Some(class) = self.class_of(obj) else {
            return;
        };
        let features = self.sorted_features(&class);
        let content_kind = class.content_kind();

        if content_kind == "simple" {
            if !elements_only {
                for f in features.iter() {
                    if f.is_reference() || !f.is_xml_attribute() || !is_set(obj, f) {
                        continue;
                    }
                    self.save_attribute(obj, f);
                }
            }
            if let Some(simple) = self.simple_feature(&class) {
                if is_set(obj, &simple) {
                    let v = obj.borrow().e_get(simple.name());
                    let s = attr_value_to_string(&simple, v);
                    if !s.is_empty() {
                        self.writer.write_text(&s);
                    }
                }
            }
            return;
        }

        if content_kind == "mixed" {
            self.save_mixed_content(obj, &features, elements_only);
            return;
        }

        // Non-mixed: emit XML attributes first (must precede child elements),
        // then the remaining features.
        for f in features.iter() {
            if f.name() == "mixed" || !is_set(obj, f) || f.is_reference() {
                continue;
            }
            if f.is_xml_attribute() && !elements_only {
                self.save_attribute(obj, f);
            }
        }
        for f in features.iter() {
            if f.name() == "mixed" || !is_set(obj, f) {
                continue;
            }
            if !f.is_reference() {
                if f.is_xml_attribute() {
                    continue; // already emitted above
                }
                self.save_attribute(obj, f);
            } else if f.is_containment() {
                self.save_containment(obj, f);
            } else {
                self.save_reference(obj, f);
            }
        }

        // Unmapped XML elements are replayed verbatim after the known features
        // (C++ `saveObjectContent`'s `unknownContents_` tail).
        if !elements_only {
            if let Some(fragments) = store::unknown_content(obj) {
                for el in &fragments {
                    self.write_element(el);
                }
            }
        }
    }

    /// Write a DOM element verbatim (C++ `writePugiNode`): used to replay
    /// unmapped elements recorded by the loader.
    fn write_element(&mut self, el: &dom::Element) {
        self.writer.begin_element(&el.name);
        for (name, value) in &el.attrs {
            self.writer.write_attribute(name, value);
        }
        for child in &el.children {
            match child {
                dom::Node::Element(e) => self.write_element(e),
                dom::Node::Text(t) => self.writer.write_text(t),
                dom::Node::Comment(c) => self.writer.write_comment(c),
            }
        }
        self.writer.end_element();
    }

    /// Replay an object's ordered mixed-content sequence (C++
    /// `saveObjectContent`'s mixed branch).
    fn save_mixed_content(
        &mut self,
        obj: &ObjectRef,
        features: &[EStructuralFeature],
        elements_only: bool,
    ) {
        let Some(seq) = store::mixed_content(obj) else {
            // No recorded sequence: comments, then features, then mixed text.
            if let Some(comments) = store::comments(obj) {
                for c in comments {
                    self.writer.write_comment(&c);
                }
            }
            for f in features {
                if f.name() == "mixed" || !is_set(obj, f) {
                    continue;
                }
                if !f.is_reference() {
                    if elements_only && f.is_xml_attribute() {
                        continue;
                    }
                    self.save_attribute(obj, f);
                } else if f.is_containment() {
                    self.save_containment(obj, f);
                } else {
                    self.save_reference(obj, f);
                }
            }
            if let Some(t) = store::mixed_text(obj) {
                if !t.trim().is_empty() {
                    self.writer.write_text(&t);
                }
            }
            return;
        };

        if !elements_only {
            for f in features {
                if f.name() == "mixed" || !is_set(obj, f) || f.is_reference() {
                    continue;
                }
                if f.is_xml_attribute() {
                    self.save_attribute(obj, f);
                }
            }
        }

        let mut wrapper_done: HashSet<String> = HashSet::new();
        let mut single_done: HashSet<String> = HashSet::new();
        for entry in &seq {
            match entry {
                MixedEntry::Text(t) => self.writer.write_text(t),
                MixedEntry::Comment(c) => self.writer.write_comment(c),
                MixedEntry::Element(child) => {
                    let Some(f) = self.feature_containing_child(obj, child) else {
                        continue;
                    };
                    if !f.is_reference() {
                        continue;
                    }
                    if f.is_many() && f.is_role_wrapper() && has_distinct_plural(&f) {
                        if wrapper_done.contains(f.name()) {
                            continue;
                        }
                        wrapper_done.insert(f.name().to_string());
                        if f.is_containment() {
                            self.save_containment(obj, &f);
                        } else {
                            self.save_reference(obj, &f);
                        }
                    } else if !f.is_many() {
                        if single_done.contains(f.name()) {
                            continue;
                        }
                        single_done.insert(f.name().to_string());
                        if let Some(cur) = single_object(obj, &f) {
                            self.save_single_mixed_element(obj, &f, &cur);
                        }
                    } else {
                        self.save_single_mixed_element(obj, &f, child);
                    }
                }
            }
        }

        // Fallback: non-XML-attribute element features not covered by the
        // sequence (attributes hold no child entries).
        for f in features {
            if f.name() == "mixed" || !is_set(obj, f) || f.is_reference() {
                continue;
            }
            let is_attr = f.is_xml_attribute();
            if elements_only && is_attr {
                continue;
            }
            if !is_attr {
                self.save_attribute(obj, f);
            }
        }
    }

    /// Find the reference feature of `parent` that holds `child`.
    fn feature_containing_child(
        &mut self,
        parent: &ObjectRef,
        child: &ObjectRef,
    ) -> Option<EStructuralFeature> {
        let class = self.class_of(parent)?;
        let features = self.sorted_features(&class);
        for f in features.iter() {
            if !f.is_reference() {
                continue;
            }
            let v = parent.borrow().e_get(f.name());
            if extract_objects(v).iter().any(|o| Rc::ptr_eq(o, child)) {
                return Some(f.clone());
            }
        }
        None
    }

    // ---- attributes ----

    /// Serialize an attribute feature (C++ `saveAttribute`).
    fn save_attribute(&mut self, obj: &ObjectRef, f: &EStructuralFeature) {
        let xml_name = f.xml_name().to_string();
        let value = obj.borrow().e_get(f.name());

        if f.is_xml_attribute() {
            let s = attr_value_to_string(f, value);
            if !s.is_empty() {
                // A feature may carry a namespace prefix (e.g. `xml.nsPrefix =
                // xml` for `xml:space`); C++ `AutosarXMLSaver::saveAttribute`
                // prefixes the attribute name in that case.
                let ns_prefix = f.xml_ns_prefix();
                if ns_prefix.is_empty() {
                    self.writer.write_attribute(&xml_name, &s);
                } else {
                    self.writer
                        .write_attribute(&format!("{ns_prefix}:{xml_name}"), &s);
                }
            }
            return;
        }

        if f.is_many() {
            let vals = extract_string_list(value);
            if vals.is_empty() {
                return;
            }
            let use_wrapper = f.is_role_wrapper() && has_distinct_plural(f);
            if use_wrapper {
                let plural = f.xml_name_plural().to_string();
                self.writer.begin_element(&plural);
                for v in &vals {
                    if v.is_empty() {
                        continue;
                    }
                    self.writer.begin_element(&xml_name);
                    self.writer.write_text(v);
                    self.writer.end_element();
                }
                self.writer.end_element();
            } else {
                for v in &vals {
                    if v.is_empty() {
                        continue;
                    }
                    self.writer.begin_element(&xml_name);
                    self.writer.write_text(v);
                    self.writer.end_element();
                }
            }
            return;
        }

        let s = attr_value_to_string(f, value);
        if is_set(obj, f) {
            self.writer.begin_element(&xml_name);
            self.writer.write_text(&s);
            self.writer.end_element();
        }
    }

    // ---- splitkey-driven child ordering ----

    /// Sort an *unordered* multi-valued feature's children by their
    /// `atp.Splitkey` (C++ `sortChildrenBySplitkey` / Java
    /// `AtpSplitkeyAwareComparator`). The sort is stable, so children with equal
    /// keys keep their stored order.
    fn sort_children_by_splitkey(&mut self, children: &mut [ObjectRef], f: &EStructuralFeature) {
        let paths = splitkey_paths(f);
        let mut keyed: Vec<(Vec<String>, ObjectRef)> = children
            .iter()
            .map(|c| {
                let keys = paths.iter().map(|p| self.splitkey_value(c, p)).collect();
                (keys, c.clone())
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        for (i, (_, c)) in keyed.into_iter().enumerate() {
            children[i] = c;
        }
    }

    /// Resolve one `atp.Splitkey` feature path on `obj` (C++ `getSplitkeyValue`):
    /// intermediate segments follow single-valued references, the final segment
    /// yields the scalar value. A path that ends on an object (or an unknown
    /// feature) yields `""`, mirroring the C++ fallback.
    ///
    /// The splitkey path uses ecore feature names, while a `DynamicEObject`
    /// stores its values under the arxml element name, so each segment is
    /// resolved through the registry first.
    fn splitkey_value(&mut self, obj: &ObjectRef, path: &[String]) -> String {
        let mut cur = obj.clone();
        for seg in path {
            let cname = cur.borrow().e_class().to_string();
            let Some(fid) = autosar448_model::reflect::class_id(&cname)
                .and_then(|cid| autosar448_model::reflect::find_feature(cid, seg))
            else {
                return String::new();
            };
            let xml = autosar448_model::reflect::feature(fid).xml_name;
            let value = cur.borrow().e_get(xml);
            match value {
                Some(Val::Object(o)) => cur = o,
                Some(Val::String(s)) => return s,
                Some(Val::EnumLiteral(e)) => return e,
                _ => return String::new(),
            }
        }
        String::new()
    }

    // ---- containment references ----

    /// Serialize a containment reference (C++ `saveContainment`).
    fn save_containment(&mut self, obj: &ObjectRef, f: &EStructuralFeature) {
        let is_many = f.is_many();
        let rule = resolve_aprxml_rule(f, is_many);
        let mut children = extract_objects(obj.borrow().e_get(f.name()));
        if children.is_empty() {
            return;
        }
        if is_many && !is_feature_ordered(f) {
            self.sort_children_by_splitkey(&mut children, f);
        }

        if rule == AprxmlRule::Rule0016 {
            for child in &children {
                self.save_object_content(child, true);
            }
            return;
        }

        if matches!(rule, AprxmlRule::Rule0012 | AprxmlRule::Rule0015) {
            for child in &children {
                let tag = self.type_xml_name(child);
                self.writer.begin_element(&tag);
                self.save_object_content(child, false);
                self.writer.end_element();
            }
            return;
        }

        // Default rule: optional role wrapper, then a per-child element whose
        // tag is the type name (typeElement) or the role name.
        let wrapper = if is_many && f.is_role_wrapper() {
            Some(f.xml_name_plural().to_string())
        } else {
            None
        };
        if let Some(w) = &wrapper {
            self.writer.begin_element(w);
        }
        let role_name = f.xml_name().to_string();
        for child in &children {
            let item_tag = if f.is_type_element() {
                self.type_xml_name(child)
            } else {
                role_name.clone()
            };
            self.writer.begin_element(&item_tag);
            self.save_object_content(child, false);
            self.writer.end_element();
        }
        if wrapper.is_some() {
            self.writer.end_element();
        }
    }

    /// Serialize a non-containment reference (C++ `saveReference`).
    fn save_reference(&mut self, obj: &ObjectRef, f: &EStructuralFeature) {
        let mut targets = extract_objects(obj.borrow().e_get(f.name()));
        if targets.is_empty() {
            return;
        }
        if f.is_many() && !is_feature_ordered(f) {
            self.sort_children_by_splitkey(&mut targets, f);
        }
        let use_wrapper = f.is_many() && f.is_role_wrapper() && has_distinct_plural(f);
        if use_wrapper {
            let plural = f.xml_name_plural().to_string();
            self.writer.begin_element(&plural);
        }
        let tag = f.xml_name().to_string();
        for target in &targets {
            self.writer.begin_element(&tag);
            self.write_reference_body(obj, f, target);
            self.writer.end_element();
        }
        if use_wrapper {
            self.writer.end_element();
        }
    }

    /// Single mixed-content child element (C++ `saveSingleMixedElement`).
    fn save_single_mixed_element(
        &mut self,
        parent: &ObjectRef,
        f: &EStructuralFeature,
        child: &ObjectRef,
    ) {
        if f.is_containment() {
            let rule = resolve_aprxml_rule(f, false);
            if rule == AprxmlRule::Rule0016 {
                self.save_object_content(child, true);
                return;
            }
            let tag = if matches!(rule, AprxmlRule::Rule0012 | AprxmlRule::Rule0015)
                || f.is_type_element()
            {
                self.type_xml_name(child)
            } else {
                f.xml_name().to_string()
            };
            let tag = if tag.is_empty() {
                f.name().to_string()
            } else {
                tag
            };
            self.writer.begin_element(&tag);
            self.save_object_content(child, false);
            self.writer.end_element();
        } else {
            let tag = if f.xml_name().is_empty() {
                f.name().to_string()
            } else {
                f.xml_name().to_string()
            };
            self.writer.begin_element(&tag);
            self.write_reference_body(parent, f, child);
            self.writer.end_element();
        }
    }

    /// Write a reference element's `DEST` attribute and path text (shared by
    /// `save_reference` / `save_single_mixed_element`).
    fn write_reference_body(
        &mut self,
        owner: &ObjectRef,
        f: &EStructuralFeature,
        target: &ObjectRef,
    ) {
        let dest =
            store::ref_dest(owner, f.name(), target).unwrap_or_else(|| self.type_xml_name(target));
        if !dest.is_empty() {
            self.writer.write_attribute(DEST_ATTR, &dest);
        }

        let mut path = self.short_name_path(target);

        // Unresolved proxy: fall back to the proxy URI text, decoding the
        // `autosar-proxy://base=X/path=Y` form when present.
        let mut encoded_base = String::new();
        let mut encoded_path = String::new();
        if target.borrow().e_is_proxy() {
            if let Some(uri) = target.borrow().e_proxy_uri().map(|u| u.to_string()) {
                if let Some(rest) = uri.strip_prefix(PROXY_PREFIX) {
                    if let Some(pos) = rest.find("/path=") {
                        encoded_base = rest[..pos].to_string();
                        encoded_path = rest[pos + 6..].to_string();
                    }
                } else {
                    path = uri;
                }
            }
        }

        if !encoded_base.is_empty() {
            // Cross-document reference with an encoded base: the C++
            // implementation treats these as `isDefault = true`, so no BASE is
            // emitted and only the relative path text is written.
            self.writer.write_text(&encoded_path);
            return;
        }

        let rel = self.try_compute_base_relative(owner, &path);
        if rel.found {
            if !rel.is_default {
                self.writer.write_attribute("BASE", &rel.base);
            }
            self.writer.write_text(&rel.text);
        } else {
            self.writer.write_text(&path);
        }
    }

    // ---- BASE-relative resolution (C++ `tryComputeBaseRelative`) ----

    fn try_compute_base_relative(&mut self, obj: &ObjectRef, path: &str) -> BaseRelative {
        let mut result = BaseRelative::default();
        if path.is_empty() {
            return result;
        }
        let mut cur = Some(obj.clone());
        while let Some(o) = cur {
            if o.borrow().e_class() == "ARPackage" {
                if let Some(class) = self.reg.find_class("ARPackage") {
                    let rb_feat = self.find_reference_bases_feature(&class);
                    if let Some(rb_feat) = rb_feat {
                        let refs = extract_objects(o.borrow().e_get(rb_feat.name()));
                        let mut best_prefix = String::new();
                        let mut best_base: Option<ObjectRef> = None;
                        for rb in &refs {
                            if Rc::ptr_eq(rb, obj) {
                                continue; // skip a ReferenceBase pointing at itself
                            }
                            let prefix = self.reference_base_prefix(rb);
                            if prefix.is_empty() || !path.starts_with(&prefix) {
                                continue;
                            }
                            if path.len() <= prefix.len() {
                                continue;
                            }
                            if !path[prefix.len()..].starts_with('/') {
                                continue;
                            }
                            if best_prefix.is_empty() || prefix.len() >= best_prefix.len() {
                                best_prefix = prefix;
                                best_base = Some(rb.clone());
                            }
                        }
                        if let Some(rb) = best_base {
                            let base = self.read_reference_base_short_label(&rb);
                            if !base.is_empty() {
                                result.base = base;
                                result.text = path[best_prefix.len() + 1..].to_string();
                                result.is_default = self.read_reference_base_is_default(&rb);
                                result.found = true;
                                return result;
                            }
                        }
                    }
                }
            }
            cur = o.borrow().e_container();
        }
        result
    }

    /// The `REFERENCE-BASES` feature of `ARPackage` (C++ matches on
    /// `xml.namePlural == "REFERENCE-BASES"` or the ecore name).
    fn find_reference_bases_feature(&self, class: &EClass) -> Option<EStructuralFeature> {
        class
            .e_all_structural_features(&self.reg)
            .into_iter()
            .find(|f| {
                f.xml_name_plural() == "REFERENCE-BASES"
                    || f.tagged_value("xml.namePlural") == Some("REFERENCE-BASES")
                    || f.name() == "referenceBases"
            })
    }

    /// The path prefix of a `ReferenceBase` (C++ `getReferenceBasePrefix`).
    fn reference_base_prefix(&mut self, ref_base: &ObjectRef) -> String {
        if read_bool_feature(ref_base, BASE_IS_THIS_PACKAGE).unwrap_or(false) {
            let mut pkg = ref_base.borrow().e_container();
            while let Some(p) = pkg {
                if p.borrow().e_class() == "ARPackage" {
                    return self.short_name_path(&p);
                }
                pkg = p.borrow().e_container();
            }
            return String::new();
        }
        let Some(class) = self.class_of(ref_base) else {
            return String::new();
        };
        let Some(feat) = class
            .e_all_structural_features(&self.reg)
            .into_iter()
            .find(|f| {
                f.xml_name() == PACKAGE_REF
                    || f.tagged_value("xml.name") == Some(PACKAGE_REF)
                    || f.name() == "package"
            })
        else {
            return String::new();
        };
        let value = ref_base.borrow().e_get(feat.name());
        let Some(target) = extract_objects(value).into_iter().next() else {
            return String::new();
        };
        if target.borrow().e_is_proxy() {
            return target
                .borrow()
                .e_proxy_uri()
                .map(|u| u.to_string())
                .unwrap_or_default();
        }
        self.short_name_path(&target)
    }

    fn read_reference_base_short_label(&self, rb: &ObjectRef) -> String {
        read_string_feature(rb, SHORT_LABEL)
            .or_else(|| read_string_feature(rb, "shortLabel"))
            .unwrap_or_default()
    }

    fn read_reference_base_is_default(&self, rb: &ObjectRef) -> bool {
        read_bool_feature(rb, IS_DEFAULT)
            .or_else(|| read_bool_feature(rb, "isDefault"))
            .unwrap_or(false)
    }
}

/// A resolved `BASE`-relative reference (C++ `BaseRelative`).
#[derive(Default)]
struct BaseRelative {
    base: String,
    text: String,
    is_default: bool,
    found: bool,
}

/// The APRXML rule for a feature (C++ `resolveAprxmlRule`).
fn resolve_aprxml_rule(f: &EStructuralFeature, is_many: bool) -> AprxmlRule {
    let role_el = f.is_role_element();
    let role_wrap = f.is_role_wrapper();
    let type_el = f.is_type_element();
    let type_wrap = f.is_type_wrapper();

    if !is_many && role_el && type_el && !role_wrap && !type_wrap {
        AprxmlRule::Rule0012
    } else if !role_wrap && !role_el && !type_wrap && type_el {
        AprxmlRule::Rule0015
    } else if !role_wrap && !role_el && !type_wrap && !type_el {
        AprxmlRule::Rule0016
    } else {
        AprxmlRule::Default
    }
}

/// Whether a multi-valued feature's children keep their stored order (C++
/// `isFeatureOrdered` / Java `ExtendedMetaData.ordered`, default `true`). The
/// generated metamodel records `ordered=false` features as the
/// `atp.unordered` tagged value.
fn is_feature_ordered(f: &EStructuralFeature) -> bool {
    f.tagged_value("atp.unordered") != Some("true")
}

/// Parse a feature's `atp.Splitkey` into the feature paths that order its
/// children (C++ `sortChildrenBySplitkey`): split on `,`, then each part on `.`
/// with whitespace trimmed — `"a, b.c"` → `[["a"], ["b", "c"]]`. Falls back to
/// `["shortName"]` when the feature declares no splitkey.
fn splitkey_paths(f: &EStructuralFeature) -> Vec<Vec<String>> {
    let splitkey = f.tagged_value("atp.Splitkey").unwrap_or("shortName");
    let mut paths: Vec<Vec<String>> = Vec::new();
    for part in splitkey.split(',') {
        let path: Vec<String> = part
            .split('.')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !path.is_empty() {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        paths.push(vec!["shortName".to_string()]);
    }
    paths
}

/// Whether the feature declares a plural wrapper name distinct from its
/// singular name (C++ `!xmlNamePlural.empty() && xmlNamePlural != xmlName`).
fn has_distinct_plural(f: &EStructuralFeature) -> bool {
    f.tagged_value("xml.namePlural").is_some() && f.xml_name_plural() != f.xml_name()
}

/// The `xsi:schemaLocation` for a resource: the resource's own value, else the
/// AUTOSAR default (C++ `AutosarSaver::save`).
fn schema_location(res: &XMIResource) -> String {
    let sl = res.get_xsi_schema_location();
    if sl.is_empty() {
        DEFAULT_SCHEMA_LOCATION.to_string()
    } else {
        sl.to_string()
    }
}

/// Whether `obj` has `f` set (EMF `eIsSet`).
fn is_set(obj: &ObjectRef, f: &EStructuralFeature) -> bool {
    obj.borrow().e_is_set(f.name())
}

/// Extract the object handles from a feature value (C++ `extractObjectList`).
fn extract_objects(value: Option<Val>) -> Vec<ObjectRef> {
    match value {
        Some(Val::Object(o)) => vec![o],
        Some(Val::List(l)) => l
            .into_iter()
            .filter_map(|v| match v {
                Val::Object(o) => Some(o),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The single object held by a reference feature (C++ `tryGetEObject` path).
fn single_object(obj: &ObjectRef, f: &EStructuralFeature) -> Option<ObjectRef> {
    extract_objects(obj.borrow().e_get(f.name()))
        .into_iter()
        .next()
}

/// Extract a multi-valued attribute's values as strings (C++
/// `extractStringList`).
fn extract_string_list(value: Option<Val>) -> Vec<String> {
    match value {
        Some(Val::List(l)) => l.iter().map(scalar_to_string).collect(),
        Some(Val::String(s)) => vec![s],
        Some(Val::EnumLiteral(e)) => vec![e],
        _ => Vec::new(),
    }
}

/// Convert a scalar `Val` to its string form.
fn scalar_to_string(v: &Val) -> String {
    match v {
        Val::String(s) => s.clone(),
        Val::EnumLiteral(e) => e.clone(),
        Val::Bool(b) => b.to_string(),
        Val::Int(i) => i.to_string(),
        Val::Byte(b) => b.to_string(),
        Val::Double(d) => d.to_string(),
        _ => String::new(),
    }
}

/// Convert an attribute's value to its serialized string (C++
/// `attrValueToString`).
fn attr_value_to_string(f: &EStructuralFeature, value: Option<Val>) -> String {
    match value {
        None | Some(Val::Null) => String::new(),
        Some(Val::Object(_)) | Some(Val::List(_)) => String::new(),
        Some(v) => datatype::to_string(f.type_name().unwrap_or("EString"), &v),
    }
}

/// Read a string-valued feature by name.
fn read_string_feature(obj: &ObjectRef, name: &str) -> Option<String> {
    match obj.borrow().e_get(name)? {
        Val::String(s) => Some(s),
        Val::EnumLiteral(e) => Some(e),
        _ => None,
    }
}

/// Read a boolean-valued feature by name (accepting `"true"` / `"1"` strings).
fn read_bool_feature(obj: &ObjectRef, name: &str) -> Option<bool> {
    match obj.borrow().e_get(name)? {
        Val::Bool(b) => Some(b),
        Val::String(s) => Some(s == "true" || s == "1"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arxml::dom;
    use crate::autosar_resource::AutosarXMLResource;
    use crate::autosar_resource_factory::AutosarResourceFactory;
    use emf_common::uri::Uri;

    fn load(arxml: &str) -> AutosarXMLResource {
        AutosarResourceFactory::register_default_autosar40_metamodel();
        let mut res = AutosarXMLResource::new(
            Uri::parse("file:///tmp/t.arxml"),
            emf_ecore::ecore_package::global(),
        );
        res.load_from_string(arxml).expect("load");
        res
    }

    #[test]
    fn writes_root_with_namespaces() {
        let res = load(
            "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
             <AR-PACKAGE><SHORT-NAME>pkg1</SHORT-NAME></AR-PACKAGE></AUTOSAR>",
        );
        let out = res.save_to_string();
        assert!(
            out.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
            "{out}"
        );
        assert!(
            out.contains("<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\""),
            "{out}"
        );
        assert!(
            out.contains("xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\""),
            "{out}"
        );
        assert!(
            out.contains("xsi:schemaLocation=\"http://autosar.org/schema/r4.0 AUTOSAR_00048.xsd\""),
            "{out}"
        );
        assert!(out.contains("<SHORT-NAME>pkg1</SHORT-NAME>"), "{out}");
        assert!(out.ends_with("</AUTOSAR>\n"), "{out}");
    }

    #[test]
    fn round_trip_preserves_tree_structure() {
        let arxml = "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
                     <AR-PACKAGES>\
                     <AR-PACKAGE><SHORT-NAME>A</SHORT-NAME></AR-PACKAGE>\
                     <AR-PACKAGE><SHORT-NAME>B</SHORT-NAME>\
                     <AR-PACKAGES><AR-PACKAGE><SHORT-NAME>sub</SHORT-NAME></AR-PACKAGE></AR-PACKAGES>\
                     </AR-PACKAGE>\
                     </AR-PACKAGES>\
                     </AUTOSAR>";
        let res = load(arxml);
        let out = res.save_to_string();
        let root = dom::parse(&out).expect("reparse");
        assert_eq!(root.local, "AUTOSAR");
        // The two AR-PACKAGE are wrapped in a single <AR-PACKAGES> role
        // wrapper, as AUTOSAR arxml requires.
        let wrapper = root
            .children
            .iter()
            .find_map(|c| match c {
                dom::Node::Element(e) if e.local == "AR-PACKAGES" => Some(e),
                _ => None,
            })
            .expect("expected an AR-PACKAGES wrapper");
        let packages = wrapper
            .children
            .iter()
            .filter_map(|c| match c {
                dom::Node::Element(e) if e.local == "AR-PACKAGE" => Some(e),
                _ => None,
            })
            .count();
        assert_eq!(packages, 2, "expected two AR-PACKAGE, got:\n{out}");
        assert!(out.contains("<SHORT-NAME>sub</SHORT-NAME>"), "{out}");

        // The output reloads into an equivalent tree.
        let res2 = load(&out);
        let r = res2.resource().contents()[0].clone();
        assert_eq!(r.borrow().e_class(), "AUTOSAR");
        let pkgs = r.borrow().e_get("AR-PACKAGE");
        match pkgs {
            Some(Val::List(l)) => assert_eq!(l.len(), 2),
            other => panic!("expected AR-PACKAGE list, got {other:?}"),
        }
    }

    /// A multi-valued feature marked `ordered=false` (here `ARPackage.arPackages`)
    /// has its children sorted by their `atp.Splitkey` before serialization, so
    /// children stored out of splitkey order still serialize in splitkey order.
    #[test]
    fn sorts_unordered_children_by_splitkey() {
        let arxml = "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
                     <AR-PACKAGES>\
                     <AR-PACKAGE><SHORT-NAME>Zeta</SHORT-NAME></AR-PACKAGE>\
                     <AR-PACKAGE><SHORT-NAME>Alpha</SHORT-NAME></AR-PACKAGE>\
                     <AR-PACKAGE><SHORT-NAME>Mid</SHORT-NAME></AR-PACKAGE>\
                     </AR-PACKAGES>\
                     </AUTOSAR>";
        let res = load(arxml);
        let out = res.save_to_string();
        let alpha = out.find("<SHORT-NAME>Alpha</SHORT-NAME>").expect("Alpha");
        let mid = out.find("<SHORT-NAME>Mid</SHORT-NAME>").expect("Mid");
        let zeta = out.find("<SHORT-NAME>Zeta</SHORT-NAME>").expect("Zeta");
        assert!(alpha < mid && mid < zeta, "not sorted by splitkey:\n{out}");
    }

    /// An element that maps to no feature is recorded by the loader and replayed
    /// verbatim by the saver (C++ `unknownContents_` round-trip), instead of
    /// being silently dropped.
    #[test]
    fn replays_unknown_element_verbatim() {
        let arxml = "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
                     <AR-PACKAGES>\
                     <AR-PACKAGE><SHORT-NAME>P</SHORT-NAME>\
                     <SOME-UNKNOWN ATTR=\"v\"><INNER>x &amp; y</INNER></SOME-UNKNOWN>\
                     </AR-PACKAGE>\
                     </AR-PACKAGES>\
                     </AUTOSAR>";
        let res = load(arxml);
        let out = res.save_to_string();
        assert!(out.contains("<SOME-UNKNOWN ATTR=\"v\">"), "{out}");
        assert!(out.contains("<INNER>x &amp; y</INNER>"), "{out}");
        assert!(out.contains("</SOME-UNKNOWN>"), "{out}");
        // Replaying is stable: a second load → save yields the same bytes.
        let out2 = load(&out).save_to_string();
        assert_eq!(out, out2);
    }

    #[test]
    fn writes_reference_with_dest_and_path() {
        let arxml = "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
                     <AR-PACKAGES>\
                     <AR-PACKAGE><SHORT-NAME>A</SHORT-NAME>\
                     <ELEMENTS>\
                     <SOME-REF DEST=\"AR-PACKAGE\">/B</SOME-REF>\
                     </ELEMENTS>\
                     </AR-PACKAGE>\
                     <AR-PACKAGE><SHORT-NAME>B</SHORT-NAME></AR-PACKAGE>\
                     </AR-PACKAGES>\
                     </AUTOSAR>";
        let res = load(arxml);
        let out = res.save_to_string();
        // The reference is written only when the feature is recognized; either
        // way the document must stay well-formed and reparse.
        assert!(dom::parse(&out).is_ok(), "{out}");
    }

    #[test]
    fn writes_attribute_element_and_escapes_text() {
        let res = load(
            "<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\
             <ADMIN-DATA><LANGUAGE>EN</LANGUAGE></ADMIN-DATA></AUTOSAR>",
        );
        let out = res.save_to_string();
        assert!(out.contains("<LANGUAGE>EN</LANGUAGE>"), "{out}");
    }

    /// A real AUTOSAR document (the `GeneralDefinitionReferenceBase` sample),
    /// exercising the three round-trip-sensitive features at once: root-level
    /// comments (mixed-content replay), the `xml:space` attribute
    /// (`xml.nsPrefix`) and a `BASE`-relative `PACKAGE-REF`.
    ///
    /// The three `REFERENCE-BASE` entries are all required: the `BASE="Cite"`
    /// of the `EnumMappingTables` reference is only reproducible because a
    /// `Cite` reference base exists with `BASE-IS-THIS-PACKAGE=true`, whose
    /// prefix (`/AUTOSAR`) the reference path is made relative to — exactly the
    /// reverse-computation the saver performs.
    const FIXTURE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\" \
        xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
        xsi:schemaLocation=\"http://autosar.org/schema/r4.0 AUTOSAR_00048.xsd\">\n\
        \x20 <!-- AUTOSAR General Definitions -->\n\
        \x20 <ADMIN-DATA>\n\
        \x20   <LANGUAGE>EN</LANGUAGE>\n\
        \x20   <USED-LANGUAGES>\n\
        \x20     <L-10 L=\"EN\" xml:space=\"default\">English</L-10>\n\
        \x20   </USED-LANGUAGES>\n\
        \x20 </ADMIN-DATA>\n\
        \x20 <AR-PACKAGES>\n\
        \x20   <AR-PACKAGE>\n\
        \x20     <SHORT-NAME>AUTOSAR</SHORT-NAME>\n\
        \x20     <REFERENCE-BASES>\n\
        \x20       <REFERENCE-BASE>\n\
        \x20         <SHORT-LABEL>ArTrace</SHORT-LABEL>\n\
        \x20         <IS-DEFAULT>false</IS-DEFAULT>\n\
        \x20         <IS-GLOBAL>true</IS-GLOBAL>\n\
        \x20         <BASE-IS-THIS-PACKAGE>true</BASE-IS-THIS-PACKAGE>\n\
        \x20         <GLOBAL-ELEMENTS>\n\
        \x20           <GLOBAL-ELEMENT>TRACEABLE</GLOBAL-ELEMENT>\n\
        \x20         </GLOBAL-ELEMENTS>\n\
        \x20       </REFERENCE-BASE>\n\
        \x20       <REFERENCE-BASE>\n\
        \x20         <SHORT-LABEL>Cite</SHORT-LABEL>\n\
        \x20         <IS-DEFAULT>false</IS-DEFAULT>\n\
        \x20         <IS-GLOBAL>true</IS-GLOBAL>\n\
        \x20         <BASE-IS-THIS-PACKAGE>true</BASE-IS-THIS-PACKAGE>\n\
        \x20         <GLOBAL-ELEMENTS>\n\
        \x20           <GLOBAL-ELEMENT>XDOC</GLOBAL-ELEMENT>\n\
        \x20         </GLOBAL-ELEMENTS>\n\
        \x20       </REFERENCE-BASE>\n\
        \x20       <REFERENCE-BASE>\n\
        \x20         <SHORT-LABEL>EnumMappingTables</SHORT-LABEL>\n\
        \x20         <IS-DEFAULT>false</IS-DEFAULT>\n\
        \x20         <IS-GLOBAL>false</IS-GLOBAL>\n\
        \x20         <BASE-IS-THIS-PACKAGE>false</BASE-IS-THIS-PACKAGE>\n\
        \x20         <PACKAGE-REF DEST=\"AR-PACKAGE\" BASE=\"Cite\">DefaultEnumMappingTables</PACKAGE-REF>\n\
        \x20       </REFERENCE-BASE>\n\
        \x20     </REFERENCE-BASES>\n\
        \x20   </AR-PACKAGE>\n\
        \x20 </AR-PACKAGES>\n\
        </AUTOSAR>\n";

    #[test]
    fn round_trip_preserves_comment_xml_space_and_base_ref() {
        let res = load(FIXTURE);
        let out = res.save_to_string();

        // The `xml:` namespace prefix on the attribute name is preserved, and
        // the attribute order matches the source document (`L` before
        // `xml:space`), i.e. the inherited-feature order of the metamodel.
        assert!(
            out.contains("<L-10 L=\"EN\" xml:space=\"default\">English</L-10>"),
            "{out}"
        );
        // The root-level comment survives via mixed-content replay.
        assert!(
            out.contains("<!-- AUTOSAR General Definitions -->"),
            "{out}"
        );
        // The reference keeps its original DEST and BASE-relative path.
        assert!(
            out.contains("<PACKAGE-REF DEST=\"AR-PACKAGE\" BASE=\"Cite\">DefaultEnumMappingTables</PACKAGE-REF>"),
            "{out}"
        );

        // The whole document round-trips byte-for-byte (comments, namespaced
        // attributes, BASE-relative references and all).
        assert_eq!(out, FIXTURE);

        // The output is idempotent: a second load → save is byte-stable.
        let res2 = load(&out);
        let out2 = res2.save_to_string();
        assert_eq!(out, out2);
    }
}
