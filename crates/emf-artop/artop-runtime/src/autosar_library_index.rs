//! `AutosarLibraryIndex` — cross-document shortName-path index for demand-load.
//!
//! Port of C++ `emf::artop::runtime::AutosarLibraryIndex` (aligned to the Java
//! ARTOP `AutosarLibraryDescriptor` + `ReferenceHelper` global path index).
//!
//! Background: arxml references use absolute shortName paths such as
//! `/AUTOSAR/AISpecification/...`. A single-file load can only index the objects
//! in that file, so cross-file references stay proxies. Java ARTOP solves this
//! with a *Library* mechanism: a pre-loaded library resource registers the
//! shortName paths of all its `GReferrable`s into a global index, which later
//! loads consult to resolve cross-document references (demand-load).
//!
//! The C++ index is a process-wide singleton keyed by `EObject*`. The Rust port
//! keeps the same process-wide singleton semantics (`instance()`), but stores
//! shared [`ObjectRef`] handles so resolved targets stay alive, and resolves the
//! shortName through EMF reflection (feature `shortName` / `SHORT-NAME`) so it is
//! metamodel-agnostic.

use emf_common::value::{ObjectRef, Val};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    /// The process-wide (thread-local) index, mirroring the C++ singleton.
    static GLOBAL: Rc<RefCell<AutosarLibraryIndex>> =
        Rc::new(RefCell::new(AutosarLibraryIndex::new()));
}

/// A path → object index for cross-document shortName resolution.
#[derive(Default)]
pub struct AutosarLibraryIndex {
    /// absolute shortName path → object.
    path_index: HashMap<String, ObjectRef>,
}

impl AutosarLibraryIndex {
    /// A fresh, empty index.
    pub fn new() -> Self {
        Self {
            path_index: HashMap::new(),
        }
    }

    /// The process-wide index (C++ `AutosarLibraryIndex::instance()`).
    ///
    /// Returns a shared handle so callers can read/mutate it directly; this keeps
    /// the C++ `instance().indexResource(res)` call shape while remaining safe in
    /// Rust's single-threaded EMF model.
    pub fn instance() -> Rc<RefCell<AutosarLibraryIndex>> {
        GLOBAL.with(Rc::clone)
    }

    /// Run `f` against the process-wide index.
    pub fn with_global<R>(f: impl FnOnce(&mut AutosarLibraryIndex) -> R) -> R {
        GLOBAL.with(|g| f(&mut g.borrow_mut()))
    }

    /// Index every root of `contents` (and their containment subtrees) by
    /// shortName path.
    pub fn index_contents(&mut self, contents: &[ObjectRef]) {
        for root in contents {
            self.index_object(root);
        }
    }

    /// Index `obj` and its containment subtree by shortName path.
    pub fn index_object(&mut self, obj: &ObjectRef) {
        if let Some(sn) = get_short_name_value(obj) {
            if !sn.is_empty() {
                let path = build_short_name_path(obj);
                if !path.is_empty() {
                    self.path_index.insert(path, Rc::clone(obj));
                }
            }
        }
        // Recurse into the containment tree (snapshot the children first so the
        // borrow ends before the recursive call re-borrows objects).
        let children = obj.borrow().e_contents();
        for c in children {
            self.index_object(&c);
        }
    }

    /// Look up an object by absolute shortName path (C++ `lookup`).
    pub fn lookup(&self, path: &str) -> Option<ObjectRef> {
        self.path_index.get(path).cloned()
    }

    /// Whether `path` is indexed (C++ `contains`).
    pub fn contains(&self, path: &str) -> bool {
        self.path_index.contains_key(path)
    }

    /// Clear the index (C++ `clear`, used to reset between tests).
    pub fn clear(&mut self) {
        self.path_index.clear();
    }

    /// Number of indexed paths (C++ `size`).
    pub fn size(&self) -> usize {
        self.path_index.len()
    }
}

/// Read an object's shortName, mirroring the C++ `getShortNameValue`:
/// try the camelCase feature first, then the ARXML element name.
fn get_short_name_value(obj: &ObjectRef) -> Option<String> {
    let o = obj.borrow();
    for name in ["shortName", "SHORT-NAME"] {
        if let Some(Val::String(s)) = o.e_get(name) {
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// Walk the `eContainer` chain collecting shortNames into an absolute path
/// `/sn1/sn2/.../snN` (aligned to Java `AutosarURIFactory.getAbsoluteQualifiedName`).
///
/// A nameless root whose class is `AUTOSAR` contributes an `AUTOSAR` segment and
/// stops the walk (`addURIFragmentSegment` for `GAUTOSAR`).
fn build_short_name_path(obj: &ObjectRef) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = Some(Rc::clone(obj));
    while let Some(node) = cur {
        let (sn, class_name, container) = {
            let b = node.borrow();
            (
                get_short_name_value(&node),
                b.e_class().to_string(),
                b.e_container(),
            )
        };
        match sn {
            Some(s) if !s.is_empty() => parts.push(s),
            _ => {
                if class_name == "AUTOSAR" || class_name == "GAUTOSAR" {
                    parts.push("AUTOSAR".to_string());
                }
                break;
            }
        }
        cur = container;
    }
    if parts.is_empty() {
        return String::new();
    }
    parts.reverse();
    let mut path = String::new();
    for p in parts {
        path.push('/');
        path.push_str(&p);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::eobject::EObject;
    use emf_ecore::{
        adopt_many, make_package_ref, node_to_object, DynNode, DynamicEObject, EClass, EClassKind,
        EPackage, EStructuralFeature, PackageRegistry,
    };
    use std::cell::RefCell as StdRefCell;

    fn registry() -> PackageRegistry {
        let mut pkg = EPackage::new("autosar40");
        pkg.set_ns_uri("http://autosar.org/schema/r4.0");

        let mut referrable = EClass::new("Referrable", EClassKind::Class);
        let mut sn = EStructuralFeature::attribute("shortName");
        sn.set_type_name("EString");
        referrable.add_feature(sn);
        pkg.add_class(referrable);

        let mut arpackage = EClass::new("ARPackage", EClassKind::Class);
        let mut sn = EStructuralFeature::attribute("shortName");
        sn.set_type_name("EString");
        arpackage.add_feature(sn);
        let mut subs = EStructuralFeature::reference_many("arPackages");
        subs.set_type_name("ARPackage");
        subs.set_containment(true);
        arpackage.add_feature(subs);
        pkg.add_class(arpackage);

        let mut autosar = EClass::new("AUTOSAR", EClassKind::Class);
        let mut pkgs = EStructuralFeature::reference_many("arPackages");
        pkgs.set_type_name("ARPackage");
        pkgs.set_containment(true);
        autosar.add_feature(pkgs);
        pkg.add_class(autosar);

        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    fn make(reg: &PackageRegistry, class: &str) -> DynNode {
        let c = reg.find_class(class).expect("class registered");
        Rc::new(StdRefCell::new(DynamicEObject::new_in(c, reg.clone())))
    }

    fn set_sn(obj: &DynNode, sn: &str) {
        obj.borrow_mut()
            .e_set("shortName", Val::String(sn.to_string()));
    }

    /// Build `<AUTOSAR><AR-PACKAGE pkg1/><AR-PACKAGE pkg2><AR-PACKAGE sub1/></AR-PACKAGE></AUTOSAR>`.
    fn sample_tree(reg: &PackageRegistry) -> (DynNode, DynNode, DynNode, DynNode) {
        let root = make(reg, "AUTOSAR");
        let p1 = make(reg, "ARPackage");
        set_sn(&p1, "pkg1");
        let p2 = make(reg, "ARPackage");
        set_sn(&p2, "pkg2");
        let sub1 = make(reg, "ARPackage");
        set_sn(&sub1, "sub1");

        // p2 owns sub1 through arPackages; root owns p1 and p2.
        adopt_many(&p2, "arPackages", &sub1);
        adopt_many(&root, "arPackages", &p1);
        adopt_many(&root, "arPackages", &p2);
        (root, p1, p2, sub1)
    }

    #[test]
    fn indexes_short_name_paths_across_containment() {
        let reg = registry();
        let (root, _, _, _) = sample_tree(&reg);
        let mut idx = AutosarLibraryIndex::new();
        idx.index_object(&node_to_object(&root));
        // root AUTOSAR + pkg1 + pkg2 + sub1
        assert!(idx.contains("/AUTOSAR/pkg1"));
        assert!(idx.contains("/AUTOSAR/pkg2"));
        assert!(idx.contains("/AUTOSAR/pkg2/sub1"));
        assert_eq!(idx.size(), 3);
    }

    #[test]
    fn lookup_returns_the_indexed_object() {
        let reg = registry();
        let (root, _, _, sub1) = sample_tree(&reg);
        let mut idx = AutosarLibraryIndex::new();
        idx.index_object(&node_to_object(&root));
        let got = idx.lookup("/AUTOSAR/pkg2/sub1").expect("sub1 indexed");
        assert!(Rc::ptr_eq(&got, &node_to_object(&sub1)));
        assert!(idx.lookup("/AUTOSAR/nope").is_none());
    }

    #[test]
    fn clear_resets_the_index() {
        let reg = registry();
        let (root, _, _, _) = sample_tree(&reg);
        let mut idx = AutosarLibraryIndex::new();
        idx.index_object(&node_to_object(&root));
        assert!(idx.size() > 0);
        idx.clear();
        assert_eq!(idx.size(), 0);
    }

    #[test]
    fn global_singleton_matches_instance_semantics() {
        AutosarLibraryIndex::with_global(|g| g.clear());
        let reg = registry();
        let (root, _, _, _) = sample_tree(&reg);
        AutosarLibraryIndex::instance()
            .borrow_mut()
            .index_object(&node_to_object(&root));
        let n = AutosarLibraryIndex::with_global(|g| g.size());
        assert_eq!(n, 3);
        AutosarLibraryIndex::with_global(|g| g.clear());
        assert_eq!(AutosarLibraryIndex::with_global(|g| g.size()), 0);
    }
}
