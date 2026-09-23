//! XMI instance-document serializer over `DynamicEObject` reflection.
//!
//! Port of the *instance-document* path of C++ `emf::xmi::XMISaver` (its
//! `writeInstance`/`saveElement` logic), aligned to Java
//! `org.eclipse.emf.ecore.xmi.impl.XMLSaveImpl`. Key behavior:
//!
//! - A single root is emitted bare; multiple roots are wrapped in `<xmi:XMI>`.
//! - Attributes become XML attributes; containment references become child
//!   elements (tagged by feature name, `xsi:type` when the concrete class
//!   differs from the declared type); non-containment references become
//!   `href` attributes.
//! - Every object gets a synthetic `xmi:id`; cross-references resolve via
//!   `//<id>`.
//!
//! This is the generic EMF `.xmi`/`.ecore` serializer. It has no knowledge of
//! any specific metamodel domain (e.g. AUTOSAR); it operates purely over
//! `DynamicEObject` reflection.

use std::cell::Ref;
use std::collections::HashMap;
use std::rc::Rc;

use emf_common::eobject::{downcast_ref, EObject};
use emf_common::value::{ObjectRef, Val};
use emf_ecore::datatype;
use emf_ecore::{
    DynamicEObject, EStructuralFeature, PackageRegistry, ECORE_NS_PREFIX, ECORE_NS_URI,
};

use super::options::XmiOptions;
use super::xml_escape::escape_attr;

/// The XMI namespace URI.
pub const XMI_NS: &str = "http://www.omg.org/XMI";
/// The XSI (XMLSchema-instance) namespace URI.
pub const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Serialize a set of root objects to an XMI XML string.
pub fn save_to_string(roots: &[ObjectRef], opts: &XmiOptions) -> String {
    let mut s = XmiSaver {
        opts,
        out: String::new(),
        ids: HashMap::new(),
        positions: index_tree_positions(roots),
        next_id: 1,
    };
    s.save(roots);
    s.out
}

struct XmiSaver<'a> {
    opts: &'a XmiOptions,
    out: String,
    /// Object pointer (Rc::as_ptr) -> xmi:id.
    ids: HashMap<usize, String>,
    /// Object pointer -> position path (`//@feat.idx`, or `//` for a root),
    /// for objects inside the saved containment trees. Cross-references to
    /// objects in a tree use this Java-compatible position path instead of an
    /// `xmi:id`.
    positions: HashMap<usize, String>,
    next_id: usize,
}

/// Index every object reachable from `roots` through containment features by
/// object pointer, mapping each to its position path (`//@feat.idx`, `//` for
/// a root). Aligned to the containment-tree walk EMF uses to build URI
/// fragments (Java XMLSave `getURIFragment`).
fn index_tree_positions(roots: &[ObjectRef]) -> HashMap<usize, String> {
    fn rec(obj: &ObjectRef, prefix: &str, out: &mut HashMap<usize, String>) {
        let key = Rc::as_ptr(obj) as *const () as usize;
        if out.contains_key(&key) {
            return; // shared object -> already positioned
        }
        out.insert(key, format!("//{prefix}"));
        let b: Ref<'_, dyn EObject> = obj.borrow();
        let dy = match downcast_ref::<DynamicEObject>(&*b) {
            Some(d) => d,
            None => return,
        };
        for f in dy.all_structural_features() {
            if !f.is_containment() {
                continue;
            }
            let name = f.name().to_string();
            let Some(val) = b.e_get(&name) else { continue };
            match val {
                Val::List(items) => {
                    for (i, item) in items.iter().enumerate() {
                        if let Some(child) = item.as_object() {
                            let seg = format!("{prefix}@{}.{}", name, i);
                            rec(&child, &seg, out);
                        }
                    }
                }
                Val::Object(child) => {
                    let seg = format!("{prefix}@{}.0", name);
                    rec(&child, &seg, out);
                }
                _ => {}
            }
        }
    }
    let mut out = HashMap::new();
    for r in roots {
        rec(r, "", &mut out);
    }
    out
}

/// A borrow-free snapshot of an object, collected so the writer never holds a
/// `RefCell` borrow while recursively emitting (which would double-borrow).
struct ObjSnap {
    class_name: String,
    prefix: String,
    ns_uri: String,
    attrs: Vec<(String, String)>,
    hrefs: Vec<(String, String)>,
    /// (feature name, declared target class, child object).
    containers: Vec<(String, Option<String>, ObjectRef)>,
}

impl<'a> XmiSaver<'a> {
    fn save(&mut self, roots: &[ObjectRef]) {
        if self.opts.xml_declaration {
            let enc = if self.opts.encoding.is_empty() {
                "UTF-8"
            } else {
                &self.opts.encoding
            };
            self.out
                .push_str(&format!("<?xml version=\"1.0\" encoding=\"{}\"?>\n", enc));
        }
        if roots.is_empty() {
            self.out.push_str(&format!(
                "<xmi:XMI {}{}=\"{}\"/>\n",
                xmi_ver_attr(self.opts),
                "xmlns:xmi=",
                XMI_NS
            ));
            return;
        }
        if self.opts.assign_ids {
            for r in roots {
                self.ensure_id(r);
            }
        }
        let wrap = roots.len() > 1;
        if wrap {
            self.out.push_str(&format!(
                "<xmi:XMI {}{}=\"{}\">\n",
                xmi_ver_attr(self.opts),
                "xmlns:xmi=",
                XMI_NS
            ));
        }
        for r in roots {
            self.write_object(r, 0, None, None);
        }
        if wrap {
            self.out.push_str("</xmi:XMI>\n");
        }
    }

    fn write_object(
        &mut self,
        obj: &ObjectRef,
        depth: usize,
        feature_tag: Option<&str>,
        declared_class: Option<&str>,
    ) {
        let snap = self.snapshot(obj);
        let ind = self.opts.indent.repeat(depth);

        // Element name: a containment child is tagged by its feature name; a
        // root is tagged prefix:ClassName.
        let tag = match feature_tag {
            Some(f) => f.to_string(),
            None => format!("{}:{}", snap.prefix, snap.class_name),
        };

        let declared_flags = feature_tag.is_some() && self.opts.declare_xsi_type;
        let needs_xsi_type = declared_flags
            && declared_class
                .map(|d| d != snap.class_name)
                .unwrap_or(false);

        // Start tag line.
        self.out.push_str(&ind);
        self.out.push('<');
        self.out.push_str(&tag);

        if depth == 0 {
            // Root: declare namespaces and xmi:version.
            if self.opts.declare_xmi {
                self.out.push_str(&format!(
                    " {}={}",
                    "xmi:version",
                    quote(&self.opts.xmi_version)
                ));
            }
            self.out
                .push_str(&format!(" {}={}", "xmlns:xmi", quote(XMI_NS)));
            self.out
                .push_str(&format!(" {}={}", "xmlns:xsi", quote(XSI_NS)));
            if !snap.prefix.is_empty() && snap.prefix != "xmi" {
                self.out
                    .push_str(&format!(" xmlns:{}={}", snap.prefix, quote(&snap.ns_uri)));
            }
        } else if needs_xsi_type {
            self.out.push_str(&format!(
                " {}={}",
                "xsi:type",
                quote(&format!("{}:{}", snap.prefix, snap.class_name))
            ));
        }

        if self.opts.assign_ids {
            let id = self.ensure_id(obj);
            self.out.push_str(&format!(" {}={}", "xmi:id", quote(&id)));
        }

        for (name, value) in &snap.attrs {
            self.out
                .push_str(&format!(" {}=\"{}\"", name, escape_attr(value)));
        }
        for (name, href) in &snap.hrefs {
            self.out
                .push_str(&format!(" {}=\"{}\"", name, escape_attr(href)));
        }

        if snap.containers.is_empty() {
            self.out.push_str("/>\n");
            return;
        }

        self.out.push_str(">\n");
        for (feat, declared_class, child) in &snap.containers {
            self.write_object(child, depth + 1, Some(feat), declared_class.as_deref());
        }
        self.out.push_str(&self.opts.indent.repeat(depth));
        self.out.push_str(&format!("</{}>\n", tag));
    }

    /// Build a borrow-free snapshot of `obj` (all owned data).
    fn snapshot(&mut self, obj: &ObjectRef) -> ObjSnap {
        let b: Ref<'_, dyn EObject> = obj.borrow();
        let class_name = b.e_class().to_string();
        let (prefix, ns_uri) = self.resolve_package(&*b, &class_name);
        let features: Vec<EStructuralFeature> = match downcast_ref::<DynamicEObject>(&*b) {
            Some(dy) => dy.all_structural_features(),
            None => Vec::new(),
        };

        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut hrefs: Vec<(String, String)> = Vec::new();
        let mut containers: Vec<(String, Option<String>, ObjectRef)> = Vec::new();

        for f in &features {
            if f.is_derived() || f.is_transient() {
                continue;
            }
            let name = f.name().to_string();
            let Some(val) = b.e_get(&name) else { continue };
            if val.is_null() {
                continue;
            }
            if f.is_reference() {
                if f.is_containment() {
                    let declared_target = f.type_name().map(|s| s.to_string());
                    for o in object_refs(&val) {
                        containers.push((name.clone(), declared_target.clone(), o));
                    }
                } else {
                    let hrs: Vec<String> = object_refs(&val)
                        .iter()
                        .map(|o| {
                            let pkey = Rc::as_ptr(o) as *const () as usize;
                            // Prefer the Java-compatible position path when the
                            // target lives inside a saved containment tree;
                            // otherwise fall back to a `//<xmi:id>` reference.
                            if let Some(path) = self.positions.get(&pkey) {
                                path.clone()
                            } else {
                                format!("//{}", self.ensure_id(o))
                            }
                        })
                        .collect();
                    if !hrs.is_empty() {
                        hrefs.push((name, hrs.join(" ")));
                    }
                }
            } else {
                let typ = f.type_name().unwrap_or("EString");
                let s = datatype::to_string(typ, &val);
                if !s.is_empty() {
                    attrs.push((name, s));
                }
            }
        }
        ObjSnap {
            class_name,
            prefix,
            ns_uri,
            attrs,
            hrefs,
            containers,
        }
    }

    /// Resolve the `(prefix, nsURI)` for an object's class via its registry.
    fn resolve_package(&self, _obj: &dyn EObject, class_name: &str) -> (String, String) {
        // Prefer the object's bound registry, else the global one.
        let reg: PackageRegistry = match downcast_ref::<DynamicEObject>(_obj) {
            Some(dy) => dy.registry().cloned(), // already an owned clone
            None => None,
        }
        .unwrap_or_else(emf_ecore::ecore_package::global);

        match reg.find_package_of_class(class_name) {
            Some(pkg) => {
                let p = pkg.borrow();
                let prefix = if p.ns_prefix().is_empty() {
                    ECORE_NS_PREFIX.to_string()
                } else {
                    p.ns_prefix().to_string()
                };
                let uri = p
                    .ns_uri()
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| ECORE_NS_URI.to_string());
                (prefix, uri)
            }
            None => (ECORE_NS_PREFIX.to_string(), ECORE_NS_URI.to_string()),
        }
    }

    /// Assign (and return) a synthetic `xmi:id` for an object.
    fn ensure_id(&mut self, obj: &ObjectRef) -> String {
        let key = Rc::as_ptr(obj) as *const () as usize;
        if let Some(id) = self.ids.get(&key) {
            return id.clone();
        }
        let id = format!("_{}", self.next_id);
        self.next_id += 1;
        self.ids.insert(key, id.clone());
        id
    }
}

fn xmi_ver_attr(opts: &XmiOptions) -> String {
    if opts.declare_xmi {
        format!("xmi:version=\"{}\" ", opts.xmi_version)
    } else {
        String::new()
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s)
}

/// Extract the referenced objects from a feature value.
fn object_refs(v: &Val) -> Vec<ObjectRef> {
    if let Some(o) = v.as_object() {
        vec![Rc::clone(o)]
    } else if let Some(l) = v.as_list() {
        l.iter()
            .filter_map(|x| x.as_object().map(Rc::clone))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::eobject::EObject;
    use emf_ecore::{
        make_package_ref, DynamicEObject, EClass, EClassKind, EStructuralFeature, PackageRegistry,
    };

    /// Build a tiny "library" metamodel: `Book { title; chapters: Chapter(*) }`
    /// and a `Chapter { name }`, registered under nsPrefix `lib`.
    fn library_package() -> (PackageRegistry, EClass, EClass) {
        let mut pkg = emf_ecore::EPackage::new("library");
        pkg.set_ns_prefix("lib");
        pkg.set_ns_uri("http://example.org/library");

        let mut chapter = EClass::new("Chapter", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        chapter.add_feature(name);

        let mut book = EClass::new("Book", EClassKind::Class);
        let mut title = EStructuralFeature::attribute("title");
        title.set_type_name("EString");
        let mut chapters = EStructuralFeature::reference_many("chapters");
        chapters.set_type_name("Chapter");
        chapters.set_containment(true);
        book.add_feature(title);
        book.add_feature(chapters);

        pkg.add_class(book);
        pkg.add_class(chapter);

        let mut registry = PackageRegistry::new();
        registry.register(make_package_ref(pkg));

        // Re-fetch the classes from the package so their features carry the
        // feature IDs assigned by `EPackage::add_class` (storage is ID-keyed).
        let book_cls = registry.find_class("Book").unwrap();
        let chapter_cls = registry.find_class("Chapter").unwrap();
        (registry, book_cls, chapter_cls)
    }

    #[test]
    fn saves_single_root_with_containment_child() {
        let (registry, book_cls, chapter_cls) = library_package();

        let chapter = std::rc::Rc::new(std::cell::RefCell::new(DynamicEObject::new_in(
            chapter_cls,
            registry.clone(),
        )));
        chapter
            .borrow_mut()
            .e_set("name", Val::String("Chapter 1".into()));

        let book = std::rc::Rc::new(std::cell::RefCell::new(DynamicEObject::new_in(
            book_cls, registry,
        )));
        book.borrow_mut()
            .e_set("title", Val::String("The Library".into()));
        book.borrow_mut()
            .e_set("chapters", Val::List(vec![Val::Object(chapter)]));

        let opts = XmiOptions::default();
        let roots: Vec<ObjectRef> = vec![book];
        let out = save_to_string(&roots, &opts);

        assert!(out.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(
            out.contains("<lib:Book "),
            "root tag should use package prefix: {out}"
        );
        assert!(
            out.contains("xmlns:lib=\"http://example.org/library\""),
            "{out}"
        );
        assert!(out.contains("title=\"The Library\""), "{out}");
        assert!(
            out.contains("<chapters "),
            "containment child tag = feature name: {out}"
        );
        assert!(out.contains("name=\"Chapter 1\""), "{out}");
        assert!(out.contains("</lib:Book>"), "{out}");
    }

    #[test]
    fn empty_roots_emit_xmi_root() {
        let opts = XmiOptions::default();
        let out = save_to_string(&[], &opts);
        assert!(out.contains("<xmi:XMI"), "{out}");
    }
}
