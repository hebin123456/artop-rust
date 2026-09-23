//! C++ parity suite: `XmiInteropTests.cpp` — Java EMF XMI interop contract.
//!
//! Ports the 5 C++ assertions that pin down C++/Java XMI cross-reference
//! compatibility:
//!   1. A non-containment reference to an object inside a containment tree is
//!      serialized as the Java-compatible position path `//@feat.idx` (never a
//!      bare `//`), and the document declares `xmlns:xsi`.
//!   2. A save → load → re-read roundtrip preserves the cross-reference.
//!   3. C++/Rust loads Java-style XMI where the cross-ref is an attribute
//!      position path `author="//@writers.0"`.
//!   4. C++/Rust loads Java-style multi-valued cross-references
//!      `authors="//@writers.0 //@writers.1"` (space-separated).
//!   5. C++/Rust loads Java-style `xmi:id`-addressed references
//!      `author="//w1"` against `<writers xmi:id="w1">`.
//!
//! Aligned to Java `XMIResourceImpl` position-path fragments (`//@feat.idx`),
//! `XMLSaveImpl.getURIFragment`, and `XMIHandler.getEObjectById`.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{make_package_ref, DynamicEObject, PackageRegistry};
use emf_ecore_codegen::loader::load_ecore_package;
use emf_xmi::XMIResource;

/// The C++ `kLibraryEcore` metamodel: `Library{name;books;writers;address}`
/// (containment), `Book{title; author→Writer non-containment}`, `Writer{name}`,
/// `Address{street}`; ns URI `http://example.com/library/1.0`, prefix
/// `library`.
const K_LIBRARY_ECORE: &str = r##"<?xml version="1.0"?>
<ecore:EPackage xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmi:version="2.0" name="library" nsURI="http://example.com/library/1.0" nsPrefix="library">
<eClassifiers xsi:type="ecore:EClass" name="Library">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="books" upperBound="-1" eType="#//Book" containment="true"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="writers" upperBound="-1" eType="#//Writer" containment="true"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="address" eType="#//Address" containment="true"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Book">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="title" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="author" eType="#//Writer"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Writer">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Address">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="street" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
</eClassifiers>
</ecore:EPackage>"##;

/// The C++ `kMultiValueEcore` metamodel for test 4: `Book.authors` is a
/// multi-valued non-containment reference to `Writer`.
const K_MULTI_ECORE: &str = r##"<?xml version="1.0"?>
<ecore:EPackage xmlns:ecore="http://www.eclipse.org/emf/2002/Ecore" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmi:version="2.0" name="lib2" nsURI="http://example.com/lib2/1.0" nsPrefix="lib2">
<eClassifiers xsi:type="ecore:EClass" name="Library">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="books" upperBound="-1" eType="#//Book" containment="true"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="writers" upperBound="-1" eType="#//Writer" containment="true"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Book">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="title" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
<eStructuralFeatures xsi:type="ecore:EReference" name="authors" upperBound="-1" eType="#//Writer"/>
</eClassifiers>
<eClassifiers xsi:type="ecore:EClass" name="Writer">
<eStructuralFeatures xsi:type="ecore:EAttribute" name="name" eType="ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"/>
</eClassifiers>
</ecore:EPackage>"##;

fn registry_from(xml: &str) -> PackageRegistry {
    let pkg = load_ecore_package(xml).unwrap();
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

fn dyn_of(reg: &PackageRegistry, class: &str) -> ObjectRef {
    let cls = reg.find_class(class).unwrap();
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
}

fn str_of(obj: &ObjectRef, feat: &str) -> String {
    match obj.borrow().e_get(feat) {
        Some(Val::String(s)) => s.to_string(),
        _ => String::new(),
    }
}

fn ref_of(obj: &ObjectRef, feat: &str) -> Option<ObjectRef> {
    obj.borrow().e_get(feat).and_then(|v| v.as_object().map(|o| o.clone()))
}

fn list_of(obj: &ObjectRef, feat: &str) -> Vec<ObjectRef> {
    match obj.borrow().e_get(feat) {
        Some(Val::List(items)) => items
            .iter()
            .filter_map(|v| v.as_object().map(|o| o.clone()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Test 1: C++/Rust output XMI contains Java-compatible position-path
/// cross-reference `author="//@writers.0"` (writer inside the containment
/// tree), never a bare `//`, and declares `xmlns:xsi`.
#[test]
fn cpp_output_has_java_compatible_href() {
    let reg = registry_from(K_LIBRARY_ECORE);

    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("My Lib".into()));

    let book = dyn_of(&reg, "Book");
    book.borrow_mut()
        .e_set("title", Val::String("B1".into()));

    let writer = dyn_of(&reg, "Writer");
    writer.borrow_mut()
        .e_set("name", Val::String("W1".into()));

    // Non-containment cross-reference: book.author -> writer.
    book.borrow_mut().e_set("author", Val::Object(writer.clone()));
    // book under lib.books (containment); writer under lib.writers.
    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(book)]));
    lib.borrow_mut()
        .e_set("writers", Val::List(vec![Val::Object(writer)]));

    let mut res = XMIResource::new(Uri::parse("file:///out.xmi"), reg);
    res.resource_mut().add_to_contents(lib);
    let out = res.save_to_string();

    assert!(
        out.contains("author=\"//@writers.0\""),
        "Java-compatible position path expected, got: {out}"
    );
    assert!(
        !out.contains("author=\"//\""),
        "no bare placeholder href allowed: {out}"
    );
    assert!(out.contains("xmlns:xsi"), "xmlns:xsi must be declared: {out}");
}

/// Test 2: roundtrip (save → load → re-read) preserves the cross-reference.
#[test]
fn roundtrip_preserves_cross_reference() {
    let reg = registry_from(K_LIBRARY_ECORE);

    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut().e_set("name", Val::String("Lib".into()));

    let book = dyn_of(&reg, "Book");
    book.borrow_mut().e_set("title", Val::String("Title".into()));

    let writer = dyn_of(&reg, "Writer");
    writer
        .borrow_mut()
        .e_set("name", Val::String("WriterName".into()));

    book.borrow_mut().e_set("author", Val::Object(writer.clone()));
    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(book)]));
    lib.borrow_mut()
        .e_set("writers", Val::List(vec![Val::Object(writer)]));

    let mut res1 = XMIResource::new(Uri::parse("file:///a.xmi"), reg);
    res1.resource_mut().add_to_contents(lib);
    let xmi1 = res1.save_to_string();

    // Reload into a fresh resource: the author cross-ref must survive.
    let reg2 = registry_from(K_LIBRARY_ECORE);
    let mut res2 = XMIResource::new(Uri::parse("file:///b.xmi"), reg2);
    res2.load_from_string(&xmi1).unwrap();

    let contents = res2.resource().contents();
    assert_eq!(contents.len(), 1, "one root after reload");
    let lib2 = &contents[0];

    let books = list_of(lib2, "books");
    assert_eq!(books.len(), 1, "one book survives the roundtrip");
    let book2 = &books[0];

    let author2 = ref_of(book2, "author").expect("author cross-ref restored");
    assert_eq!(
        str_of(&author2, "name"),
        "WriterName",
        "author targets the original writer"
    );
}

/// Test 3: load Java-style XMI with an attribute position-path cross-reference
/// `author="//@writers.0"`.
#[test]
fn load_java_style_attribute_cross_reference() {
    let reg = registry_from(K_LIBRARY_ECORE);

    let java_style = r#"<?xml version="1.0"?>
<library:Library xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:library="http://example.com/library/1.0" name="Java Lib">
  <books title="Java Book" author="//@writers.0"/>
  <writers name="Java Writer"/>
</library:Library>"#;

    let mut res = XMIResource::new(Uri::parse("file:///java.xmi"), reg);
    res.load_from_string(java_style).unwrap();
    let lib = &res.resource().contents()[0];

    let books = list_of(lib, "books");
    assert_eq!(books.len(), 1);
    assert_eq!(str_of(&books[0], "title"), "Java Book");

    let author = ref_of(&books[0], "author").expect("author resolved");
    assert_eq!(str_of(&author, "name"), "Java Writer");
}

/// Test 4: load Java-style multi-valued attribute cross-references
/// `authors="//@writers.0 //@writers.1"` into a list of two writers.
#[test]
fn load_java_style_multi_value_attribute_cross_reference() {
    let reg = registry_from(K_MULTI_ECORE);

    let java_style = r#"<?xml version="1.0"?>
<lib2:Library xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:lib2="http://example.com/lib2/1.0" name="Multi Lib">
  <books title="B1" authors="//@writers.0 //@writers.1"/>
  <writers name="W1"/>
  <writers name="W2"/>
</lib2:Library>"#;

    let mut res = XMIResource::new(Uri::parse("file:///multi.xmi"), reg);
    res.load_from_string(java_style).unwrap();
    let lib = &res.resource().contents()[0];
    assert_eq!(str_of(lib, "name"), "Multi Lib");

    let books = list_of(lib, "books");
    let book = &books[0];

    let authors = list_of(book, "authors");
    assert_eq!(authors.len(), 2, "multi-value cross-ref yields two writers");
    assert_eq!(str_of(&authors[0], "name"), "W1");
    assert_eq!(str_of(&authors[1], "name"), "W2");
}

/// Test 5: load Java-style XMI with an `xmi:id`-addressed cross-reference
/// `author="//w1"` against `<writers xmi:id="w1">`.
#[test]
fn load_java_style_xmiid_reference() {
    let reg = registry_from(K_LIBRARY_ECORE);

    let java_style = r#"<?xml version="1.0"?>
<library:Library xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:library="http://example.com/library/1.0" name="ID Lib">
  <books title="B1" author="//w1"/>
  <writers xmi:id="w1" name="WriterByID"/>
</library:Library>"#;

    let mut res = XMIResource::new(Uri::parse("file:///id.xmi"), reg);
    res.load_from_string(java_style).unwrap();
    let lib = &res.resource().contents()[0];

    let books = list_of(lib, "books");
    assert_eq!(books.len(), 1);

    let author = ref_of(&books[0], "author").expect("xmi:id cross-ref resolved");
    assert_eq!(str_of(&author, "name"), "WriterByID");
}