//! Shared helpers for `emf-compare` integration tests.
//!
//! This module is re-included by every integration test crate, but each one only
//! exercises a subset of the builders/readers below, so unused-code warnings are
//! expected and silenced.
#![allow(dead_code)]
//!
//! Mirrors the in-memory `DynamicEObject` graph construction used by the C++
//! `CompareE2ETests.cpp` / `CompareP0RegressionTests.cpp` (which loaded the
//! `Ecore` models from XMI strings). The Rust port builds the same models
//! programmatically with `DynamicEObject` + `adopt_many` instead of parsing XMI.
//!
//! Graphs are built at the [`DynNode`] level (so containment back-links resolve
//! correctly) and only the roots are handed back as [`ObjectRef`]-wrapped
//! roots for the compare engines.
//!
//! Models:
//! - **Library** (`Library(name, books)`, `Book(title, pages)`) — E2E reachability/merge.
//! - **Shop / Item** (`Shop(items)`, `Item(id, name)` with `id` = ID attrs) — P0 auto-ID.
//! - **LibAuthor** (`Library(books, authors)`, `Book(title, author)`,
//!   `Author(name)`) — P0 reference/map.
//! - **Bidirectional** (`Parent(children)` eOpposite `Child.parent`) — P0 eOpposite.

use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{adopt_many, node_to_object, DynNode, DynamicEObject, EClass, EClassKind, EStructuralFeature};
use std::cell::RefCell;
use std::rc::Rc;

/// Build a single-valued attribute feature.
fn attr(name: &str, fid: i32) -> EStructuralFeature {
    let mut f = EStructuralFeature::attribute(name);
    f.set_type_name(name);
    f.set_feature_id(fid);
    f
}

/// Build an ID attribute (EMF `EAttribute` with `iD=true`).
fn attr_id(name: &str, fid: i32) -> EStructuralFeature {
    let mut f = attr(name, fid);
    f.set_id(true);
    f
}

/// Build a many-valued containment reference feature (optionally with an
/// `eOpposite` feature name).
fn ref_many(name: &str, fid: i32, opposite: Option<&str>) -> EStructuralFeature {
    let mut f = EStructuralFeature::reference_many(name);
    f.set_containment(true);
    f.set_feature_id(fid);
    if let Some(opp) = opposite {
        f.set_opposite(opp);
    }
    f
}

/// Build a single-valued non-containment reference feature.
fn ref_single(name: &str, fid: i32) -> EStructuralFeature {
    let mut f = EStructuralFeature::reference(name);
    f.set_feature_id(fid);
    f
}

/// A fresh concrete `DynNode` of a class.
fn node(class: EClass) -> DynNode {
    Rc::new(RefCell::new(DynamicEObject::new(class)))
}

const fn contained(_nodes: &[DynNode]) {}

// ===== Library model =====

pub struct LibraryMeta {
    pub lib_cls: EClass,
    pub book_cls: EClass,
}

pub fn library_meta() -> LibraryMeta {
    // Library.name (0), Library.books (1, many containment),
    // Book.title (0), Book.pages (1).
    let mut lib = EClass::new("Library", EClassKind::Class);
    lib.add_feature(attr("name", 0));
    lib.add_feature(ref_many("books", 1, None));
    let mut book = EClass::new("Book", EClassKind::Class);
    book.add_feature(attr("title", 0));
    book.add_feature(attr("pages", 1));
    LibraryMeta {
        lib_cls: lib,
        book_cls: book,
    }
}

/// `Library(name) -> [Book(title,pages), ...]`, returned as an `ObjectRef`.
pub fn build_library(m: &LibraryMeta, name: &str, books: &[(String, i32)]) -> ObjectRef {
    let lib = node(m.lib_cls.clone());
    lib.borrow_mut().e_set("name", Val::String(name.into()));
    for (title, pages) in books {
        let book = node(m.book_cls.clone());
        book.borrow_mut().e_set("title", Val::String(title.clone()));
        book.borrow_mut().e_set("pages", Val::Int(*pages as i64));
        adopt_many(&lib, "books", &book);
    }
    node_to_object(&lib)
}

// ===== Shop / Item model =====

pub struct ShopMeta {
    pub shop_cls: EClass,
    pub item_cls: EClass,
}

pub fn shop_meta() -> ShopMeta {
    // Shop.items (0, many containment), Item.id (0, ID), Item.name (1).
    let mut shop = EClass::new("Shop", EClassKind::Class);
    shop.add_feature(ref_many("items", 0, None));
    let mut item = EClass::new("Item", EClassKind::Class);
    item.add_feature(attr_id("id", 0));
    item.add_feature(attr("name", 1));
    item.set_id_feature(0); // the "id" attribute is the ID feature.
    ShopMeta {
        shop_cls: shop,
        item_cls: item,
    }
}

/// `Shop -> [Item(id,name), ...]`, returned as an `ObjectRef`.
pub fn build_shop(m: &ShopMeta, items: &[(String, String)]) -> ObjectRef {
    let shop = node(m.shop_cls.clone());
    for (id, name) in items {
        let it = node(m.item_cls.clone());
        it.borrow_mut().e_set("id", Val::String(id.clone()));
        it.borrow_mut().e_set("name", Val::String(name.clone()));
        adopt_many(&shop, "items", &it);
    }
    node_to_object(&shop)
}

// ===== LibAuthor model =====

pub struct LibAuthorMeta {
    pub lib_cls: EClass,
    pub book_cls: EClass,
    pub author_cls: EClass,
}

pub fn lib_author_meta() -> LibAuthorMeta {
    // Library.books (0, many containment), Library.authors (1, many
    // containment), Book.title (0), Book.author (1, single non-containment),
    // Author.name (0).
    let mut lib = EClass::new("Library", EClassKind::Class);
    lib.add_feature(ref_many("books", 0, None));
    lib.add_feature(ref_many("authors", 1, None));
    let mut book = EClass::new("Book", EClassKind::Class);
    book.add_feature(attr("title", 0));
    book.add_feature(ref_single("author", 1));
    let mut author = EClass::new("Author", EClassKind::Class);
    author.add_feature(attr("name", 0));
    LibAuthorMeta {
        lib_cls: lib,
        book_cls: book,
        author_cls: author,
    }
}

/// `Library { books=[Book(title=title, author=Author(name))], authors=[Author(name)] }`.
pub fn build_lib_author(m: &LibAuthorMeta, author_name: &str, title: &str) -> ObjectRef {
    let lib = node(m.lib_cls.clone());
    let book = node(m.book_cls.clone());
    book.borrow_mut().e_set("title", Val::String(title.into()));
    let author = node(m.author_cls.clone());
    author.borrow_mut().e_set("name", Val::String(author_name.into()));
    adopt_many(&lib, "books", &book);
    adopt_many(&lib, "authors", &author);
    book.borrow_mut().e_set("author", Val::Object(node_to_object(&author)));
    node_to_object(&lib)
}

/// `Library { books=[Book(title=title, author)], authors=[Alice(, Bob)] }`;
/// when `add_bob` is true the book's author is Bob (a second Author).
pub fn build_lib_author_bob(m: &LibAuthorMeta, title: &str, add_bob: bool) -> ObjectRef {
    let lib = node(m.lib_cls.clone());
    let book = node(m.book_cls.clone());
    book.borrow_mut().e_set("title", Val::String(title.into()));
    let alice = node(m.author_cls.clone());
    alice.borrow_mut().e_set("name", Val::String("Alice".into()));
    adopt_many(&lib, "authors", &alice);
    let mut author_for_book = node_to_object(&alice);
    if add_bob {
        let bob = node(m.author_cls.clone());
        bob.borrow_mut().e_set("name", Val::String("Bob".into()));
        adopt_many(&lib, "authors", &bob);
        author_for_book = node_to_object(&bob);
    }
    book.borrow_mut().e_set("author", Val::Object(author_for_book));
    adopt_many(&lib, "books", &book);
    node_to_object(&lib)
}

// ===== Bidirectional Parent/Child model =====

pub struct BidirMeta {
    pub parent_cls: EClass,
    pub child_cls: EClass,
}

pub fn bidir_meta() -> BidirMeta {
    // Parent.name (0), Parent.children (1, many containment,
    //   eOpposite=Child/parent), Child.name (0), Child.parent (1, single
    //   non-containment transient, eOpposite=Parent/children).
    let mut parent = EClass::new("Parent", EClassKind::Class);
    parent.add_feature(attr("name", 0));
    parent.add_feature(ref_many("children", 1, Some("parent")));
    let mut child = EClass::new("Child", EClassKind::Class);
    child.add_feature(attr("name", 0));
    let mut parent_ref = ref_single("parent", 1);
    parent_ref.set_transient(true);
    child.add_feature(parent_ref);
    BidirMeta {
        parent_cls: parent,
        child_cls: child,
    }
}

/// `Parent(name)` with an optional child named `child_name`.
pub fn build_parent(m: &BidirMeta, name: &str, child_name: Option<&str>) -> ObjectRef {
    let parent = node(m.parent_cls.clone());
    parent.borrow_mut().e_set("name", Val::String(name.into()));
    if let Some(cname) = child_name {
        let child = node(m.child_cls.clone());
        child.borrow_mut().e_set("name", Val::String(cname.into()));
        adopt_many(&parent, "children", &child);
    }
    node_to_object(&parent)
}

// ===== generic read helpers used by assertions =====

/// Read a string feature value.
pub fn read_str(o: &ObjectRef, name: &str) -> Option<String> {
    o.borrow().e_get(name).and_then(|v| v.as_str().map(String::from))
}

/// Read an integer feature value.
pub fn read_int(o: &ObjectRef, name: &str) -> Option<i64> {
    o.borrow().e_get(name).and_then(|v| v.as_int())
}

/// Read a specific (single- or many-valued) reference feature as a list. This
/// mirrors `EObject::eGet(ref) -> EList<EObject*>` used throughout the C++
/// tests, and distinguishes one containment feature from another (unlike the
/// flattened `children`).
pub fn ref_list(o: &ObjectRef, name: &str) -> Vec<ObjectRef> {
    match o.borrow().e_get(name) {
        Some(Val::List(items)) => items.iter().filter_map(|v| v.as_object().cloned()).collect(),
        Some(Val::Object(x)) => vec![x.clone()],
        _ => Vec::new(),
    }
}

/// The containment children of an object (via `e_contents`).
pub fn children(o: &ObjectRef) -> Vec<ObjectRef> {
    o.borrow().e_contents()
}

/// Number of containment children (used to size-check lists after merge).
pub fn child_count(o: &ObjectRef) -> usize {
    children(o).len()
}