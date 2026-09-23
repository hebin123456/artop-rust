//! Rust port parity tests for `ValidationE2ETests.cpp`
//! (C++ `emf-validation`, aligned to EMF Validation's static full-tree
//! `validateAll` + dynamic `LiveValidator`).
//!
//! Models are built manually with `DynamicEObject` (no XMI loading):
//! `Library(name, books: Book[] containment)` -> `Book(title, pages,
//! author: Writer containment)` -> `Writer(name)`.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::diagnostic::Diagnostic;
use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;
use emf_ecore::{
    adopt_many, node_to_object, DynamicEObject, DynNode, EClass, EClassKind, EStructuralFeature, Val,
};
use emf_validation::e_validator::EValidator;
use emf_validation::live_validator::ValidationLiveAdapter;
use emf_validation::validation_service::ValidationService;

/// The `Library` EClass: `name` (EString) + `books` (containment, many).
fn library_class() -> EClass {
    let mut lib = EClass::new("Library", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    lib.add_feature(name);
    let mut books = EStructuralFeature::reference_many("books");
    books.set_type_name("Book");
    books.set_containment(true);
    lib.add_feature(books);
    lib
}

/// The `Book` EClass: `title`, `pages` (attributes) + `author: Writer`
/// (single containment reference whose `lowerBound` is configurable — pass `1`
/// to make it a *required* reference).
fn book_class(author_lower_bound: i32) -> EClass {
    let mut book = EClass::new("Book", EClassKind::Class);
    let mut title = EStructuralFeature::attribute("title");
    title.set_type_name("EString");
    book.add_feature(title);
    let mut pages = EStructuralFeature::attribute("pages");
    pages.set_type_name("EInt");
    book.add_feature(pages);
    let mut author = EStructuralFeature::reference("author"); // single-valued
    author.set_type_name("Writer");
    author.set_containment(true);
    author.set_lower_bound(author_lower_bound);
    book.add_feature(author);
    book
}

/// The `Writer` EClass: `name` (EString). Declared as the `author` target type
/// but never instantiated in these scenarios (books carry no author).
fn writer_class() -> EClass {
    let mut writer = EClass::new("Writer", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    writer.add_feature(name);
    writer
}

/// Build a `Library` containing two books, with `Book.author.lowerBound` set to
/// `author_lower_bound`. `author` is left unset (so when `author_lower_bound
/// >= 1` it is a required-but-null violation). The `Writer` class is built to
/// keep the meta-model complete, matching the C++ scene.
fn make_library(author_lower_bound: i32, name: &str) -> DynNode {
    let _writer = writer_class();
    let lib: DynNode = Rc::new(RefCell::new(DynamicEObject::new(library_class())));
    lib.borrow_mut().e_set("name", Val::string(name));
    let book_cls = book_class(author_lower_bound);

    for (title, pages) in [("Book One", 100i64), ("Book Two", 200i64)] {
        let book: DynNode = Rc::new(RefCell::new(DynamicEObject::new(book_cls.clone())));
        book.borrow_mut().e_set("title", Val::string(title));
        book.borrow_mut().e_set("pages", Val::Int(pages));
        adopt_many(&lib, "books", &book);
    }
    lib
}

/// A `ValidationService` preloaded with the default built-in constraints
/// (batch + live twins), matching the C++ `ValidationService` constructor which
/// calls `registerDefaultConstraints`.
fn default_service() -> ValidationService {
    let mut svc = ValidationService::new();
    svc.validator().register_default_constraints();
    svc
}

// ===== 测试 1：静态全量校验 valid 模型 → 0 diagnostic =====
#[test]
fn static_all_valid_model_has_no_diagnostics() {
    let lib = make_library(0, "Test Library");
    let svc = default_service();
    let diags = svc.validate_all(&*lib.borrow());
    assert_eq!(diags.len(), 0);
}

// ===== 测试 2：静态全量校验 空 name → 产 NoEmptyName diagnostic =====
#[test]
fn static_all_empty_name_produces_no_empty_name() {
    let lib = make_library(0, "Test Library");
    lib.borrow_mut().e_set("name", Val::string(""));

    let svc = default_service();
    let diags = svc.validate_all(&*lib.borrow());
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.source().contains("NoEmptyName")));
}

// ===== 测试 3：静态全量校验 缺 required reference → 产 NoNullRequiredRef =====
#[test]
fn static_all_null_required_ref_produces_no_null_required_ref() {
    // Book.author lowerBound == 1 and left unset -> the required ref is null.
    let lib = make_library(1, "Test Library");

    let svc = default_service();
    let diags = svc.validate_all(&*lib.borrow());
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.source().contains("NoNullRequiredRef")));
}

// ===== 测试 4：动态变更校验 — eSet name="" 触发 LiveValidator listener =====
//
// Trade-off vs C++ `ValidationLiveAdapter.attach`: the Rust `DynamicEObject`
// emits no change notifications (and mutating an object while re-reading it from
// inside its own `RefCell::borrow_mut` would deadlock), so this adapter drives
// live validation explicitly via `validate_now` after each mutation — the same
// convention already used by `artop-validation`. The `attach`/`detach`/
// `add_listener` plumbing and the listener dispatch are exercised end-to-end.
#[test]
fn live_attach_set_empty_name_triggers_listener() {
    let lib_node = make_library(0, "Test Library");
    let lib: ObjectRef = node_to_object(&lib_node);

    let mut validator = EValidator::new();
    validator.register_default_constraints();
    let mut live = ValidationLiveAdapter::new(validator);

    // Collect every diagnostic the listener receives.
    let received: Rc<RefCell<Vec<Diagnostic>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let rec = Rc::clone(&received);
        live.add_listener(Box::new(move |_target: &dyn EObject, diags: &[Diagnostic]| {
            rec.borrow_mut().extend(diags.iter().cloned());
        }));
    }

    // attach to the Library (root).
    live.attach(lib.clone());

    // A valid state produces no diagnostics.
    lib_node.borrow_mut().e_set("name", Val::string("Still Valid"));
    let _ = live.validate_now(&*lib_node.borrow());
    assert_eq!(received.borrow().len(), 0);

    // Setting name to "" must produce a NoEmptyName diagnostic via the listener.
    lib_node.borrow_mut().e_set("name", Val::string(""));
    let _ = live.validate_now(&*lib_node.borrow());
    assert!(
        received
            .borrow()
            .iter()
            .any(|d| d.source().contains("NoEmptyName"))
    );

    live.detach();
}