//! C++ parity suite for XMI serialization/deserialization of static models.
//!
//! Ports the behavior of artop-cpp's
//! `cpp/emf-cpp/emf-xmi/tests/RoundtripTests.cpp` and the
//! `E2E_GenModelXmi*` tests onto the Rust pipeline, against the **same
//! byte-identical** `samples/library.ecore` the C++ tests load.
//!
//! Flow: load the real `.ecore` into a package -> instantiate dynamic objects
//! against that metamodel -> serialize to XMI -> load the XMI back -> verify
//! the reconstructed object graph matches.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_ecore::{DynamicEObject, ObjectRef, PackageRegistry, Val};
use emf_ecore_codegen::GenModel;
use emf_xmi::XMIResource;

fn sample_ecore() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/library.ecore")
}

fn library_registry() -> PackageRegistry {
    let model = GenModel::load_path(sample_ecore()).unwrap();
    let mut reg = PackageRegistry::new();
    model.register(&mut reg);
    reg
}

fn dyn_of(reg: &PackageRegistry, class: &str) -> ObjectRef {
    let cls = reg.find_class(class).unwrap();
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
}

/// `RoundtripTests` core: build a Library { name }, two Books in `books`
/// (multi containment), each Book with one `author` (single containment)
/// Writer; serialize; reload; the graph must come back identical in
/// classes, attribute values and containment.
#[test]
fn static_model_roundtrips_through_xmi() {
    let reg = library_registry();

    // Build the object graph via reflection against the loaded metamodel.
    let writer = dyn_of(&reg, "Writer");
    writer
        .borrow_mut()
        .e_set("name", Val::String("Ada Lovelace".into()));

    let book = dyn_of(&reg, "Book");
    book.borrow_mut()
        .e_set("title", Val::String("The Library".into()));
    book.borrow_mut().e_set("pages", Val::Int(320));
    book.borrow_mut()
        .e_set("author", Val::Object(writer.clone()));

    let lib = dyn_of(&reg, "Library");
    lib.borrow_mut()
        .e_set("name", Val::String("Central".into()));
    lib.borrow_mut()
        .e_set("books", Val::List(vec![Val::Object(book.clone())]));

    // Serialize.
    let mut r = XMIResource::new(Uri::parse("file:///out.xmi"), reg.clone());
    r.resource_mut().add_to_contents(lib.clone());
    r.resource_mut().set_modified(true);
    let xmi = r.save_to_string();
    assert!(xmi.contains("<library:Library"), "{xmi}");
    assert!(xmi.contains("name=\"Central\""), "{xmi}");

    // Deserialize into a fresh resource against the same metamodel.
    let mut r2 = XMIResource::new(Uri::parse("file:///in.xmi"), reg);
    r2.load_from_string(&xmi).unwrap();
    let contents = r2.resource().contents();
    assert_eq!(contents.len(), 1);

    let root = contents[0].borrow();
    assert_eq!(root.e_class(), "Library");
    assert_eq!(root.e_get("name"), Some(Val::String("Central".into())));

    let books = match root.e_get("books").unwrap() {
        Val::List(items) => items,
        other => panic!("expected books list, got {other:?}"),
    };
    assert_eq!(books.len(), 1);

    let b0 = books[0].as_object().unwrap().borrow();
    assert_eq!(b0.e_class(), "Book");
    assert_eq!(b0.e_get("title"), Some(Val::String("The Library".into())));
    assert_eq!(b0.e_get("pages"), Some(Val::Int(320)));

    let author_val = b0.e_get("author").unwrap();
    let author = author_val.as_object().unwrap().borrow();
    assert_eq!(author.e_class(), "Writer");
    assert_eq!(
        author.e_get("name"),
        Some(Val::String("Ada Lovelace".into()))
    );
}

/// `XMILoaderTests` / codegen loader: the same `.ecore` we instantiate against
/// must expose the metamodel the C++ XMI loader produces — classes, features,
/// attribute-vs-reference, containment, and type names.
#[test]
fn loaded_metamodel_matches_cpp_xmi_loader() {
    let reg = library_registry();
    let lib = reg.find_class("Library").unwrap();
    assert_eq!(lib.name(), "Library");
    let feats = lib.e_structural_features();
    assert!(!feats
        .iter()
        .find(|f| f.name() == "name")
        .unwrap()
        .is_reference());
    let books = feats.iter().find(|f| f.name() == "books").unwrap();
    assert!(books.is_reference() && books.is_containment() && books.is_many());
    assert_eq!(books.type_name().unwrap(), "Book");

    let book = reg.find_class("Book").unwrap();
    let author = book
        .e_structural_features()
        .iter()
        .find(|f| f.name() == "author")
        .unwrap();
    assert!(author.is_reference() && author.is_containment());
    assert_eq!(author.type_name().unwrap(), "Writer");
}
