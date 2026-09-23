//! XMI instance-document loader over `DynamicEObject` reflection.
//!
//! The inverse of [`super::saver`]: parses an XMI/XML string into an element
//! tree ([`super::parser`]) and reflects it back into `DynamicEObject`s using a
//! [`PackageRegistry`] to resolve every class and feature. Behavior mirrored
//! from the instance-document path of C++ `emf::xmi::XMLHandler` /
//! `emf::xmi::XMIHelper`:
//!
//! - A root element `<prefix:Class>` yields an object of `Class`.
//! - Containment children (element tag = a containment reference's name) are
//!   created with the declared type from the feature, overridden by `xsi:type`
//!   when present.
//! - Attributes map by feature name and are parsed with `datatype::from_string`;
//!   `xmlns*`, `xsi:type`, `xmi:id`, `xmi:version` are structural and skipped.
//! - Local cross-references (`href="//<xmi:id>"`) are resolved against the
//!   document's `xmi:id` table after all objects are built.

use std::cell::Ref;
use std::collections::HashMap;
use std::rc::Rc;

use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::datatype;
use emf_ecore::{DynamicEObject, EClass, EStructuralFeature, PackageRegistry};

use super::parser::{parse, XmlNode};

/// Load a complete XMI/XML document into `DynamicEObject`s. Returns the object
/// roots in document order.
pub fn load_from_str(src: &str, registry: &PackageRegistry) -> Result<Vec<ObjectRef>, String> {
    load_from_str_with_ids(src, registry).map(|(roots, _)| roots)
}

/// Load a document like [`load_from_str`], additionally returning the table of
/// `xmi:id` -> object entries harvested while building the graph. Used so a
/// resource can expose `getEObjectByID` for ids that appear in the document.
pub fn load_from_str_with_ids(
    src: &str,
    registry: &PackageRegistry,
) -> Result<(Vec<ObjectRef>, HashMap<String, ObjectRef>), String> {
    let roots = parse(src)?;
    let mut id_map: HashMap<String, ObjectRef> = Default::default();
    let mut deferred: Vec<(ObjectRef, String, String)> = Vec::new();
    let mut built = Vec::new();

    // Unwrap the optional <xmi:XMI> document wrapper so its element children
    // become roots (the wrapper element itself is not a model object).
    let top_level = unwrap_xmi_wrapper(&roots);

    for node in &top_level {
        let obj = build_node(node, registry, &mut id_map, &mut deferred, None)?;
        built.push(obj);
    }
    resolve_hrefs(&built, &id_map, &mut deferred);
    Ok((built, id_map))
}

/// Index every object reachable from `roots` through containment features by
/// its position path: roots at `""`, and each child at `@feat.idx` (nested
/// e.g. `@books.0/@chapters.1`). Aligned to the containment-tree walk EMF uses
/// to build `//@feat.idx` URI fragments. Returns `path -> object`.
fn index_positions(roots: &[ObjectRef]) -> HashMap<String, ObjectRef> {
    let mut out = HashMap::new();
    for r in roots {
        // Track visited pointers so a shared object isn't re-walked into an
        // infinite recursion (shared containment is unusual but legal).
        let mut seen: std::collections::HashSet<usize> = Default::default();
        rec_positions(r, "", &mut out, &mut seen);
    }
    out
}

/// DFS from a root, recording each reachable object's position path.
fn rec_positions(
    obj: &ObjectRef,
    path: &str,
    out: &mut HashMap<String, ObjectRef>,
    seen: &mut std::collections::HashSet<usize>,
) {
    let key = Rc::as_ptr(obj) as *const () as usize;
    if !seen.insert(key) {
        return;
    }
    out.entry(path.to_string()).or_insert_with(|| Rc::clone(obj));
    let b: Ref<'_, dyn EObject> = obj.borrow();
    let dy = match emf_common::eobject::downcast_ref::<DynamicEObject>(&*b) {
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
                        let seg = format!("{path}@{}.{}", name, i);
                        rec_positions(&child, &seg, out, seen);
                    }
                }
            }
            Val::Object(child) => {
                let seg = format!("{path}@{}.0", name);
                rec_positions(&child, &seg, out, seen);
            }
            _ => {}
        }
    }
}

/// Return `roots` unchanged, unless there is a single wrapper element whose
/// local name is `XMI` and prefix `xmi` — in which case its children are the
/// real roots.
fn unwrap_xmi_wrapper(roots: &[XmlNode]) -> Vec<XmlNode> {
    if roots.len() == 1 {
        let n = &roots[0];
        if n.prefix.as_deref() == Some("xmi") && n.local == "XMI" {
            return n.children.clone();
        }
    }
    roots.to_vec()
}

/// Build one element (a root or a containment child) into an object.
fn build_node(
    node: &XmlNode,
    registry: &PackageRegistry,
    id_map: &mut std::collections::HashMap<String, ObjectRef>,
    deferred: &mut Vec<(ObjectRef, String, String)>,
    parent: Option<&EStructuralFeature>,
) -> Result<ObjectRef, String> {
    let class = resolve_class(node, registry, parent)?;
    let obj: ObjectRef = Rc::new(std::cell::RefCell::new(DynamicEObject::new_in(
        class,
        registry.clone(),
    )));

    if let Some(id) = node.attr("xmi:id") {
        id_map
            .entry(id.to_string())
            .or_insert_with(|| Rc::clone(&obj));
    }

    // Group containment children by feature so multi-valued references are set
    // as a list once all siblings are known.
    let mut container_lists: std::collections::HashMap<String, Vec<ObjectRef>> = Default::default();

    for (aname, avalue) in &node.attrs {
        if is_structural_attr(aname) {
            continue;
        }
        let local = local_part(aname);
        let Some(feat) = find_feature(&obj, &local) else {
            continue; // unknown feature: record-and-skip semantics
        };
        if !feat.is_reference() {
            let typ = feat.type_name().unwrap_or("EString");
            let val = datatype::from_string(typ, avalue);
            obj.borrow_mut().e_set(&local, val);
        } else if !feat.is_containment() {
            // non-containment cross-reference; defer until ids are known
            deferred.push((Rc::clone(&obj), local.to_string(), avalue.clone()));
        }
    }

    for child in &node.children {
        let Some(feat) = find_feature(&obj, &child.local) else {
            continue; // unknown containment tag: skip
        };
        if !feat.is_containment() {
            continue;
        }
        let child_obj = build_node(child, registry, id_map, deferred, Some(&feat))?;
        if feat.is_many() {
            container_lists
                .entry(feat.name().to_string())
                .or_default()
                .push(child_obj);
        } else {
            obj.borrow_mut().e_set(feat.name(), Val::Object(child_obj));
        }
    }
    for (fname, list) in container_lists {
        let vals: Vec<Val> = list.into_iter().map(Val::Object).collect();
        obj.borrow_mut().e_set(&fname, Val::List(vals));
    }

    Ok(obj)
}

/// Resolve the class for an element. Containment children are typed by the
/// declared feature type unless `xsi:type="prefix:Class"` overrides it; roots
/// are typed by the element's local name.
fn resolve_class(
    node: &XmlNode,
    registry: &PackageRegistry,
    parent: Option<&EStructuralFeature>,
) -> Result<EClass, String> {
    let class_name = match parent {
        Some(feat) => match node.attr("xsi:type") {
            Some(t) => local_part(t).to_string(),
            None => feat
                .type_name()
                .map(|t| t.to_string())
                .unwrap_or_else(|| node.local.clone()),
        },
        None => node.local.clone(),
    };
    registry
        .find_class(&class_name)
        .ok_or_else(|| format!("no class '{}' in registry", class_name))
}

/// Whether an attribute name is structural XMI/XSI/XMLNS metadata and should be
/// skipped when reflecting feature values.
fn is_structural_attr(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.starts_with("xmlns")
        || lower == "xmi:id"
        || lower == "xmi:version"
        || lower == "xmi:uuid"
        || lower == "xsi:type"
}

/// Find a structural feature of `obj` by (local) feature name.
fn find_feature(obj: &ObjectRef, name: &str) -> Option<EStructuralFeature> {
    let b: Ref<'_, dyn EObject> = obj.borrow();
    let dy = emf_common::eobject::downcast_ref::<DynamicEObject>(&*b)?;
    dy.all_structural_features()
        .into_iter()
        .find(|f| f.name() == name)
}

/// After all objects are built, resolve deferred cross-references against the
/// built containment tree and the document's `xmi:id` table.
///
/// Supported href forms (Java EMF inline cross-reference contract):
/// - `//<xmi:id>`: an object addressed by its `xmi:id` (aligned to
///   `XMIHandler.getEObjectById`).
/// - `//@feat.idx[{(/@feat.idx)...}]`: a position path from a root through a
///   containment feature by index (aligned to `XMIResource.resolvePositionPath`).
/// - Multiple of the above separated by whitespace for multi-valued
///   non-containment references (Java serializes each element separately).
fn resolve_hrefs(
    built: &[ObjectRef],
    id_map: &std::collections::HashMap<String, ObjectRef>,
    deferred: &mut Vec<(ObjectRef, String, String)>,
) {
    // Position paths are resolved against the containment tree of the built
    // roots, indexed once up front for repeated lookups.
    let positions = index_positions(built);

    for (owner, feat, href) in deferred.drain(..) {
        for token in href.split_whitespace() {
            if let Some(target) = resolve_token(token, id_map, &positions) {
                append_reference(&owner, &feat, target);
            }
        }
    }
}

/// Resolve one cross-reference token to an object, or `None` on failure.
fn resolve_token(
    token: &str,
    id_map: &std::collections::HashMap<String, ObjectRef>,
    positions: &HashMap<String, ObjectRef>,
) -> Option<ObjectRef> {
    if let Some(rest) = token.strip_prefix("//") {
        if rest.starts_with('@') {
            // Position path: `@feat.idx/@other.0...` from a root.
            return resolve_position(rest, positions);
        }
        // `//<id>`: strip the slashes and look up the xmi:id.
        return id_map.get(rest).cloned();
    }
    None
}

/// Resolve a `@feat.idx/{@feat.idx...}` path to the object carrying exactly
/// that position in the indexed containment tree (EMF `resolvePositionPath`).
fn resolve_position(path: &str, positions: &HashMap<String, ObjectRef>) -> Option<ObjectRef> {
    // Index-keyed direct lookup: everything in the containment tree was
    // indexed up front, so any nested `@a.0/@b.1` path resolves in one step.
    positions.get(path).cloned()
}

/// Append a target to a reference feature: wrap in a list for multi-valued,
/// overwrite for single-valued.
fn append_reference(obj: &ObjectRef, feat: &str, target: ObjectRef) {
    let many = {
        let b = obj.borrow();
        match emf_common::eobject::downcast_ref::<DynamicEObject>(&*b) {
            Some(dy) => dy
                .all_structural_features()
                .into_iter()
                .find(|f| f.name() == feat)
                .map(|f| f.is_many())
                .unwrap_or(false),
            None => return,
        }
    };
    if many {
        let existing = { obj.borrow().e_get(feat) };
        let new_list = match existing {
            Some(Val::List(mut list)) => {
                list.push(Val::Object(target));
                list
            }
            _ => vec![Val::Object(target)],
        };
        obj.borrow_mut().e_set(feat, Val::List(new_list));
    } else {
        obj.borrow_mut().e_set(feat, Val::Object(target));
    }
}

fn local_part(qname: &str) -> &str {
    match qname.rfind(':') {
        Some(idx) => &qname[idx + 1..],
        None => qname,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::XmiOptions;
    use crate::saver::save_to_string;
    use emf_ecore::{make_package_ref, EClass, EClassKind, EStructuralFeature};

    /// Reuse the small "library" metamodel from the saver tests.
    fn library_registry() -> PackageRegistry {
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
        registry
    }

    fn new_object(class_name: &str, registry: &PackageRegistry) -> ObjectRef {
        let class = registry.find_class(class_name).unwrap();
        Rc::new(std::cell::RefCell::new(DynamicEObject::new_in(
            class,
            registry.clone(),
        )))
    }

    /// saver -> loader roundtrip: what the saver writes must load back with the
    /// same class, attributes and containment structure.
    #[test]
    fn roundtrip_saver_to_loader() {
        let registry = library_registry();

        let chapter = new_object("Chapter", &registry);
        chapter
            .borrow_mut()
            .e_set("name", Val::String("Chapter 1".into()));

        let book = new_object("Book", &registry);
        book.borrow_mut()
            .e_set("title", Val::String("The Library".into()));
        book.borrow_mut()
            .e_set("chapters", Val::List(vec![Val::Object(chapter)]));

        let opts = XmiOptions::default();
        let xmi = save_to_string(std::slice::from_ref(&book), &opts);

        let loaded = load_from_str(&xmi, &registry).unwrap();
        assert_eq!(loaded.len(), 1);
        let lroot = &loaded[0];
        assert_eq!(lroot.borrow().e_class(), "Book");
        assert_eq!(
            lroot.borrow().e_get("title"),
            Some(Val::String("The Library".into()))
        );
        // containment child should be reconstructed
        let chapters = lroot.borrow().e_get("chapters").unwrap();
        let rest: Vec<ObjectRef> = match &chapters {
            Val::List(items) => items
                .iter()
                .filter_map(|v| v.as_object().map(|o| (*o).clone()))
                .collect(),
            _ => vec![],
        };
        assert_eq!(
            rest.len(),
            1,
            "one containment chapter expected, got {chapters:?}"
        );
        assert_eq!(rest[0].borrow().e_class(), "Chapter");
        assert_eq!(
            rest[0].borrow().e_get("name"),
            Some(Val::String("Chapter 1".into()))
        );
    }

    #[test]
    fn loads_single_root_with_attributes() {
        let registry = library_registry();
        let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<lib:Book xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:lib="http://example.org/library" title="Hello"/>
"#;
        let loaded = load_from_str(src, &registry).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].borrow().e_class(), "Book");
        assert_eq!(
            loaded[0].borrow().e_get("title"),
            Some(Val::String("Hello".into()))
        );
        // `chapters` is a containment reference that was never populated.
        assert!(!loaded[0].borrow().e_is_set("chapters"));
    }

    #[test]
    fn resolves_local_href_cross_reference() {
        let registry = library_registry();
        let src = r#"
<xmi:XMI xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:lib="http://example.org/library">
  <lib:Book xmi:id="_1" title="A"/>
  <lib:Book xmi:id="_2" title="B" related="_1"/>
</xmi:XMI>
"#;
        // "related" is unknown in the metamodel; loader must skip it without
        // error, still loading both roots.
        let loaded = load_from_str(src, &registry).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].borrow().e_class(), "Book");
        assert_eq!(loaded[1].borrow().e_class(), "Book");
    }
}
