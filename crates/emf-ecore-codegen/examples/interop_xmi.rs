//! 双向 XMI 互操作 harness 的 Rust 半侧。
//!
//! 与 C++ 侧 `tools/conformance/interop_xmi_main.cpp` 构建同一份 `library`
//! 元模型 + 同一份实例数据，由 `tools/conformance/interop_xmi.py` 驱动，证明两个
//! 移植版本能互相读写对方的 XMI 文件。
//!
//!   cargo run -p emf-ecore-codegen --example interop_xmi -- emit      <out.xmi>
//!   cargo run -p emf-ecore-codegen --example interop_xmi -- check     <in.xmi>
//!   cargo run -p emf-ecore-codegen --example interop_xmi -- roundtrip <in.xmi> <out.xmi>

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{make_package_ref, DynamicEObject, PackageRegistry};
use emf_ecore_codegen::loader::load_ecore_package;
use emf_xmi::XMIResource;

/// 与 C++ 侧 `kLibraryEcore` 完全一致的元模型。
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

// 两侧约定的实例数据。
const LIB_NAME: &str = "Interop Lib";
const BOOK_TITLE: &str = "B1";
const WRITER_NAME: &str = "W1";

fn registry() -> Result<PackageRegistry, String> {
    let pkg = load_ecore_package(K_LIBRARY_ECORE)?;
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    Ok(reg)
}

fn dyn_of(reg: &PackageRegistry, class: &str) -> ObjectRef {
    let cls = reg.find_class(class).expect("class registered");
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
}

fn str_of(obj: &ObjectRef, feat: &str) -> String {
    match obj.borrow().e_get(feat) {
        Some(Val::String(s)) => s.to_string(),
        _ => String::new(),
    }
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

fn ref_of(obj: &ObjectRef, feat: &str) -> Option<ObjectRef> {
    obj.borrow()
        .e_get(feat)
        .and_then(|v| v.as_object().map(|o| o.clone()))
}

fn read_file(path: &str) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) => Err(format!("reading `{path}`: {e}")),
    }
}

fn write_file(path: &str, data: &str) -> Result<(), String> {
    if let Err(e) = std::fs::write(path, data) {
        return Err(format!("writing `{path}`: {e}"));
    }
    Ok(())
}

fn emit(out: &str) -> Result<(), String> {
    let reg = registry()?;

    let lib = dyn_of(&reg, "Library");
    let name = Val::String(LIB_NAME.into());
    lib.borrow_mut().e_set("name", name);

    let book = dyn_of(&reg, "Book");
    let title = Val::String(BOOK_TITLE.into());
    book.borrow_mut().e_set("title", title);

    let writer = dyn_of(&reg, "Writer");
    let wname = Val::String(WRITER_NAME.into());
    writer.borrow_mut().e_set("name", wname);

    // Book.author 是非 containment 跨引用；writer 同时挂在 lib.writers（containment）
    // 下，因此序列化应产出 Java 兼容的位置路径 `//@writers.0`。
    let author = Val::Object(writer.clone());
    book.borrow_mut().e_set("author", author);

    let books = Val::List(vec![Val::Object(book)]);
    lib.borrow_mut().e_set("books", books);

    let writers = Val::List(vec![Val::Object(writer)]);
    lib.borrow_mut().e_set("writers", writers);

    let mut res = XMIResource::new(Uri::parse("file:///interop.xmi"), reg);
    res.resource_mut().add_to_contents(lib);
    let xmi = res.save_to_string();
    write_file(out, &xmi)?;
    println!("EMIT-OK {out}");
    Ok(())
}

fn load(in_path: &str) -> Result<XMIResource, String> {
    let src = read_file(in_path)?;
    let reg = registry()?;
    let mut res = XMIResource::new(Uri::parse("file:///interop.xmi"), reg);
    if let Err(e) = res.load_from_string(&src) {
        return Err(format!("loading `{in_path}`: {e}"));
    }
    Ok(res)
}

fn check(in_path: &str) -> Result<(), String> {
    let res = load(in_path)?;
    let contents = res.resource().contents();
    if contents.len() != 1 {
        return Err(format!("expected 1 root, got {}", contents.len()));
    }
    let lib = &contents[0];

    let got_name = str_of(lib, "name");
    if got_name != LIB_NAME {
        return Err(format!("root name `{got_name}` != `{LIB_NAME}`"));
    }

    let books = list_of(lib, "books");
    if books.len() != 1 {
        return Err(format!("expected exactly 1 book, got {}", books.len()));
    }
    let book = &books[0];
    let got_title = str_of(book, "title");
    if got_title != BOOK_TITLE {
        return Err(format!("book title `{got_title}` != `{BOOK_TITLE}`"));
    }

    let author = match ref_of(book, "author") {
        Some(a) => a,
        None => return Err("author cross-reference not resolved".to_string()),
    };
    let got_author = str_of(&author, "name");
    if got_author != WRITER_NAME {
        return Err(format!("author name `{got_author}` != `{WRITER_NAME}`"));
    }

    println!("CHECK-OK {in_path}");
    Ok(())
}

fn roundtrip(in_path: &str, out_path: &str) -> Result<(), String> {
    let res = load(in_path)?;
    let xmi = res.save_to_string();
    write_file(out_path, &xmi)?;
    println!("ROUNDTRIP-OK {out_path}");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.len() {
        3 if args[1] == "emit" => emit(&args[2]),
        3 if args[1] == "check" => check(&args[2]),
        4 if args[1] == "roundtrip" => roundtrip(&args[2], &args[3]),
        _ => {
            eprintln!("usage: interop_xmi <emit|check> <file.xmi>");
            eprintln!("       interop_xmi roundtrip <in.xmi> <out.xmi>");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("INTEROP-FAIL: {e}");
        std::process::exit(1);
    }
}
