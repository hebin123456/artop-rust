//! C++ parity suite: `resourceset_multi_file_test.cpp` — Scenario A contract
//! (ordinary XMI multi-file, ported; Scenario B is arxml/autosar and gated).
//!
//! Scenario A semantics: `a.xmi` holds `EPackage` `pkgA` with `EClass A`;
//! `b.xmi` holds `pkgB` with `EClass B` where `eSuperTypes="a.xmi#//A"` cross-
//! references `A`. After both files are loaded and registered together,
//! `B.eSuperTypes` resolves to the real `EClass A` (non-proxy) rather than a
//! dangling reference.
//!
//! Rust mapping: each metamodel file is loaded by
//! `emf_ecore_codegen::loader::load_ecore_package` into an `EPackage`; both
//! are registered in one [`PackageRegistry`]. Cross-document super types are
//! stored by name and resolve to a real `EClass` found across the combined
//! registry. (EMF also demand-loads the referenced file through the
//! `ResourceSet` and resolves by object identity; the Rust metamodel layer has
//! no persistent proxy objects, so the resolution-equivalent is a name that
//! now points at a genuinely registered classifier.)

use emf_ecore::PackageRegistry;

const A_XMI: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<ecore:EPackage xmi:version=\"2.0\" xmlns:xmi=\"http://www.omg.org/XMI\" ",
    "xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\" name=\"pkgA\" ",
    "nsURI=\"http://example.com/pkgA\" nsPrefix=\"pkgA\">\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"A\"/>\n",
    "</ecore:EPackage>\n"
);

const B_XMI: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<ecore:EPackage xmi:version=\"2.0\" xmlns:xmi=\"http://www.omg.org/XMI\" ",
    "xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\" name=\"pkgB\" ",
    "nsURI=\"http://example.com/pkgB\" nsPrefix=\"pkgB\">\n",
    "  <eClassifiers xsi:type=\"ecore:EClass\" name=\"B\" eSuperTypes=\"a.xmi#//A\"/>\n",
    "</ecore:EPackage>\n"
);

#[test]
fn multi_file_supertype_resolves_across_packages() {
    // Each file loads into its own EPackage, in isolation.
    let pkg_a = emf_ecore_codegen::loader::load_ecore_package(A_XMI).unwrap();
    let pkg_b = emf_ecore_codegen::loader::load_ecore_package(B_XMI).unwrap();
    assert_eq!(pkg_a.name(), "pkgA");
    assert_eq!(pkg_a.ns_prefix(), "pkgA");
    assert_eq!(pkg_b.name(), "pkgB");
    assert_eq!(pkg_b.ns_prefix(), "pkgB");

    // [A1] pkgA declares EClass "A".
    let cls_a = pkg_a
        .find_class("A")
        .expect("a.xmi must declare EClass A");
    assert_eq!(cls_a.name(), "A");

    // [A3/A4] B.eSuperTypes references a.xmi#//A -> recorded as super type "A".
    let b = pkg_b.find_class("B").expect("b.xmi must declare EClass B");
    assert!(!b.e_super_types().is_empty(), "B must declare a super type");
    assert_eq!(b.e_super_types()[0], "A", "cross-doc href tail is the class name");
    let sup = b.e_super_types()[0].clone();

    // Register both packages into ONE registry and resolve the reference to a
    // real EClass (the Rust equivalent of EMF's "non-proxy resolved").
    let reg = {
        let mut reg = PackageRegistry::new();
        reg.register(std::rc::Rc::new(std::cell::RefCell::new(pkg_a)));
        reg.register(std::rc::Rc::new(std::cell::RefCell::new(pkg_b)));
        reg
    };
    let resolved = reg
        .find_class(&sup)
        .expect("super type must resolve to a real EClass (non-proxy)");
    assert_eq!(resolved.name(), "A");
    assert_eq!(resolved.e_structural_features().len(), 0);
}