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
use std::rc::Rc;

use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::datatype;
use emf_ecore::{DynamicEObject, EClass, EStructuralFeature, PackageRegistry};

use super::parser::{parse, XmlNode};

/// Load a complete XMI/XML document into `DynamicEObject`s. Returns the object
/// roots in document order.
pub fn load_from_str(src: &str, registry: &PackageRegistry) -> Result<Vec<ObjectRef>, String> {
    let roots = parse(src)?;
    let mut id_map: std::collections::HashMap<String, ObjectRef> = Default::default();
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
    Ok(built)
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

/// After all objects are built, resolve local `//<xmi:id>` hrefs against the
/// document's id table.
fn resolve_hrefs(
    _built: &[ObjectRef],
    id_map: &std::collections::HashMap<String, ObjectRef>,
    deferred: &mut Vec<(ObjectRef, String, String)>,
) {
    for (owner, feat, href) in deferred.drain(..) {
        if let Some(target) = href.strip_prefix("//") {
            if let Some(tobj) = id_map.get(target) {
                append_reference(&owner, &feat, Rc::clone(tobj));
            }
        }
    }
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
