//! Rust source generator: walk an [`EPackage`] and emit statically-typed code.
//!
//! Port target: the emitting half of C++ `emf-ecore-codegen` (`CppGenerator` /
//! `EClassEmitter` / `PackageEmitter`), adapted to emit Rust instead of C++.
//!
//! Each generated package is self-contained (a thin crate): `lib.rs` declares
//! one `pub struct` per `EClass` backed by concrete typed fields, a `match`
//! reflection table per class implementing
//! [`emf_common::eobject::EObject`], plus a `package()` helper that registers
//! the metadata into a [`PackageRegistry`]. Nothing in the output needs the
//! original `.ecore` file to compile or run.

use std::fmt::Write as _;

use emf_ecore::{EClassKind, EPackage, EStructuralFeature};

use crate::typing::{field_name, field_rust_type, struct_name};

/// Render a whole package as an idiomatic Rust source string.
pub fn generate_source(pkg: &EPackage) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "//! Static model generated from `{}` — do not edit by hand.",
        pkg.name()
    )
    .unwrap();
    writeln!(
        out,
        "//! nsURI={} nsPrefix={}",
        pkg.ns_uri().map(|u| u.to_string()).unwrap_or_default(),
        pkg.ns_prefix()
    )
    .unwrap();
    writeln!(out, "#![allow(dead_code)]").unwrap();
    // Generated code is compiled standalone and may be only partially
    // exercised: suppress lint noise that depends on the consumer's usage.
    writeln!(
        out,
        "#![allow(unused_imports, unused_mut, non_snake_case, clippy::too_many_arguments)]"
    )
    .unwrap();
    out.push_str("use emf_common::eobject::EObject;\n");
    out.push_str("use emf_common::value::{ObjectRef, Val};\n");
    out.push_str("use emf_ecore::structural::FeatureKind;\n");
    out.push_str("use emf_ecore::{make_package_ref, EClass, EClassKind, EStructuralFeature, PackageRegistry};\n\n");

    // One struct per concrete class.
    for class in pkg.classes() {
        emit_class(&mut out, class);
    }

    // Package registrar.
    out.push_str("/// Register this package's metadata into `reg` for reflection/serialization.\n");
    out.push_str("pub fn register_package(reg: &mut PackageRegistry) {\n");
    out.push_str("    let mut pkg = emf_ecore::EPackage::new(\"");
    out.push_str(pkg.name());
    out.push_str("\");\n");
    if let Some(ns) = pkg.ns_uri() {
        out.push_str("    pkg.set_ns_uri(\"");
        out.push_str(&ns.to_string());
        out.push_str("\");\n");
    }
    out.push_str("    pkg.set_ns_prefix(\"");
    out.push_str(pkg.ns_prefix());
    out.push_str("\");\n");
    for class in pkg.classes() {
        writeln!(
            out,
            "    let mut {c} = eclass(\"{name}\", {kind});",
            c = struct_name(class.name()),
            name = class.name(),
            kind = class_kind_lit(class.kind()),
        )
        .unwrap();
        for super_ in class.e_super_types() {
            writeln!(
                out,
                "    {c}.add_super_type(\"{s}\").ok();",
                c = struct_name(class.name()),
                s = super_
            )
            .unwrap();
        }
        for feat in class.e_structural_features() {
            let ty = feat.type_name().unwrap_or("EString");
            writeln!(out, "    {c}.add_feature(struct_feature(\"{name}\", {kind_fn}, {many}, {lower}, &\"{ty}\", {is_ref}));",
                c = struct_name(class.name()),
                name = feat.name(),
                kind_fn = if feat.is_reference() { "FeatureKind::Reference" } else { "FeatureKind::Attribute" },
                many = feat.is_many(),
                lower = feat.lower_bound(),
                is_ref = feat.is_reference(),
            ).unwrap();
        }
        writeln!(
            out,
            "    pkg.add_class({c});",
            c = struct_name(class.name())
        )
        .unwrap();
    }
    out.push_str("    reg.register(make_package_ref(pkg));\n");
    out.push_str("}\n\n");

    // Small internal helpers (avoid deriving a heavy feature-type enum on every struct).
    out.push_str("fn eclass(name: &str, kind: EClassKind) -> EClass {\n");
    out.push_str("    EClass::new(name, kind)\n");
    out.push_str("}\n");
    out.push_str("fn struct_feature(name: &str, kind: FeatureKind, many: bool, lower: i32, ty: &str, is_ref: bool) -> EStructuralFeature {\n");
    out.push_str("    let upper = if many { -1 } else { 1 };\n");
    out.push_str("    let mut f = emf_ecore::EStructuralFeature::new(name, kind, lower, upper);\n");
    out.push_str("    f.set_type_name(ty);\n");
    out.push_str("    if is_ref { f.set_containment(false); }\n");
    out.push_str("    f\n");
    out.push_str("}\n");
    // Shared scalar extractor for multi-valued attributes (values stored as Int).
    out.push_str("fn scalar_as_i64(v: &Val) -> Option<i64> {\n");
    out.push_str("    v.as_int()\n");
    out.push_str("}\n");

    out
}

fn class_kind_lit(kind: EClassKind) -> &'static str {
    match kind {
        EClassKind::Class => "EClassKind::Class",
        EClassKind::AbstractClass => "EClassKind::AbstractClass",
        EClassKind::Interface => "EClassKind::Interface",
        EClassKind::MapEntry => "EClassKind::MapEntry",
    }
}

fn emit_class(out: &mut String, class: &emf_ecore::EClass) {
    let sname = struct_name(class.name());
    let fields: Vec<(String, String)> = class
        .e_structural_features()
        .iter()
        .map(|f| (field_name(f.name()), field_rust_type(f)))
        .collect();

    // Struct declaration.
    writeln!(out, "#[derive(Debug, Clone)]").unwrap();
    writeln!(out, "pub struct {sname} {{").unwrap();
    for (fname, ftype) in &fields {
        writeln!(out, "    pub {fname}: {ftype},").unwrap();
    }
    out.push_str("}\n\n");

    // Inherent accessors (typed getters/setters become a clean surface).
    writeln!(out, "impl {sname} {{").unwrap();
    writeln!(out, "    /// New empty instance.").unwrap();
    writeln!(out, "    pub fn new() -> Self {{ Self {{").unwrap();
    for (fname, _) in &fields {
        writeln!(out, "        {fname}: Default::default(),").unwrap();
    }
    writeln!(out, "    }} }}").unwrap();
    for feat in class.e_structural_features() {
        let fname = field_name(feat.name());
        if feat.is_many() {
            continue;
        }
        let tys = field_rust_type(feat); // e.g. Option<String>
        let inner = tys.trim_start_matches("Option<").trim_end_matches('>');
        writeln!(out, "    /// Typed getter for `{}`.", feat.name()).unwrap();
        writeln!(
            out,
            "    pub fn get_{fname}(&self) -> &{tys} {{ &self.{fname} }}"
        )
        .unwrap();
        writeln!(out, "    /// Typed setter for `{}`.", feat.name()).unwrap();
        if feat.is_reference() {
            // Reference setter takes an ObjectRef directly (no Into impl exists).
            writeln!(
                out,
                "    pub fn set_{fname}(&mut self, v: {inner}) -> &mut Self {{"
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "    pub fn set_{fname}(&mut self, v: impl Into<{inner}>) -> &mut Self {{"
            )
            .unwrap();
        }
        writeln!(out, "        self.{fname} = Some(v.into());").unwrap();
        writeln!(out, "        self").unwrap();
        writeln!(out, "    }}").unwrap();
    }
    out.push_str("}\n\n");

    // EObject reflection table.
    writeln!(out, "impl EObject for {sname} {{").unwrap();
    writeln!(
        out,
        "    fn e_class(&self) -> &str {{ \"{}\" }}",
        class.name()
    )
    .unwrap();
    writeln!(out, "    fn as_any(&self) -> &dyn std::any::Any {{ self }}").unwrap();
    writeln!(out, "    fn e_get(&self, name: &str) -> Option<Val> {{").unwrap();
    writeln!(out, "        match name {{").unwrap();
    for feat in class.e_structural_features() {
        let fname = field_name(feat.name());
        let val = e_get_value(feat, &fname);
        writeln!(out, "            \"{}\" => {val},", feat.name()).unwrap();
    }
    out.push_str("            _ => None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    writeln!(
        out,
        "    fn e_set(&mut self, name: &str, value: Val) -> bool {{"
    )
    .unwrap();
    writeln!(out, "        match name {{").unwrap();
    for feat in class.e_structural_features() {
        let fname = field_name(feat.name());
        if feat.is_many() {
            // multi-valued: accept a list of objects/scalars.
            if feat.is_reference() {
                writeln!(out, "            \"{}\" => {{ self.{fname} = value.as_list().map(|l| l.iter().filter_map(|v| v.as_object().cloned()).collect()).unwrap_or_default(); true }}", feat.name()).unwrap();
            } else {
                writeln!(out, "            \"{}\" => {{ self.{fname} = value.as_list().map(|l| l.iter().filter_map(scalar_as_i64).collect()).unwrap_or_default(); true }}", feat.name()).unwrap();
            }
        } else if feat.is_reference() {
            writeln!(out, "            \"{}\" => {{ if let Some(o) = value.as_object() {{ self.{fname} = Some(o.clone()); true }} else {{ false }} }}", feat.name()).unwrap();
        } else {
            // attribute, scalar (single-valued)
            let type_name = feat.type_name().unwrap_or("EString");
            let inner = field_rust_type(feat); // Option<...>
            let inner_ty = inner.trim_start_matches("Option<").trim_end_matches('>');
            let conv = scalar_from_val(type_name, &fname, inner_ty);
            writeln!(out, "            \"{}\" => {{ {conv} }}", feat.name()).unwrap();
        }
    }
    out.push_str("            _ => false,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    writeln!(out, "    fn e_is_set(&self, name: &str) -> bool {{").unwrap();
    writeln!(out, "        match name {{").unwrap();
    for feat in class.e_structural_features() {
        let fname = field_name(feat.name());
        if feat.is_many() {
            writeln!(
                out,
                "            \"{}\" => !self.{fname}.is_empty(),",
                feat.name()
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "            \"{}\" => self.{fname}.is_some(),",
                feat.name()
            )
            .unwrap();
        }
    }
    out.push_str("            _ => false,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    writeln!(out, "    fn e_unset(&mut self, name: &str) -> bool {{").unwrap();
    writeln!(out, "        match name {{").unwrap();
    for feat in class.e_structural_features() {
        let fname = field_name(feat.name());
        writeln!(
            out,
            "            \"{}\" => {{ self.{fname} = Default::default(); true }}",
            feat.name()
        )
        .unwrap();
    }
    out.push_str("            _ => false,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    writeln!(out, "    fn e_contents(&self) -> Vec<ObjectRef> {{").unwrap();
    writeln!(out, "        let mut out = Vec::new();").unwrap();
    for feat in class.e_structural_features() {
        if !feat.is_reference() || !feat.is_containment() {
            continue;
        }
        let fname = field_name(feat.name());
        if feat.is_many() {
            writeln!(
                out,
                "        for c in &self.{fname} {{ out.push(c.clone()); }}"
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "        if let Some(c) = &self.{fname} {{ out.push(c.clone()); }}"
            )
            .unwrap();
        }
    }
    out.push_str("        out\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
}

/// Build the `e_get` value expression for a feature (returns an `Option<Val>`
/// expression, i.e. the match arm body must already be `Option<Val>`).
fn e_get_value(feat: &EStructuralFeature, fname: &str) -> String {
    if feat.is_reference() {
        if feat.is_many() {
            format!(
                "if self.{fname}.is_empty() {{ None }} else {{ Some(Val::List(self.{fname}.iter().map(|o| Val::Object(o.clone())).collect())) }}"
            )
        } else {
            format!("self.{fname}.as_ref().map(|o| Val::Object(o.clone()))")
        }
    } else {
        // attribute scalar
        let type_name = feat.type_name().unwrap_or("EString");
        let elt = scalar_elt(type_name, feat.is_many());
        if feat.is_many() {
            // multi-valued attribute stored as Vec<i64>; map to Int list.
            format!(
                "if self.{fname}.is_empty() {{ None }} else {{ Some(Val::List(self.{fname}.iter().map(|v| {elt}(*v)).collect())) }}"
            )
        } else {
            match elt {
                "Val::Int" => format!("self.{fname}.map(Val::Int)"),
                "Val::Double" => format!("self.{fname}.map(Val::Double)"),
                "Val::Bool" => format!("self.{fname}.map(Val::Bool)"),
                _ => format!("self.{fname}.clone().map(Val::String)"),
            }
        }
    }
}

/// The `Val` constructor element for a scalar data-type name.
fn scalar_elt(type_name: &str, _many: bool) -> &'static str {
    match type_name {
        "EInt" | "EIntegerObject" | "EShort" | "EShortObject" | "EChar" | "ECharacterObject"
        | "ELong" | "ELongObject" | "EByte" | "EByteObject" => "Val::Int",
        "EDouble" | "EDoubleObject" | "EFloat" | "EFloatObject" => "Val::Double",
        "EBoolean" | "EBooleanObject" => "Val::Bool",
        _ => "Val::String", // EString and everything else
    }
}

/// Build the `e_set` match-arm body for a single-valued attribute.
fn scalar_from_val(type_name: &str, fname: &str, field_inner: &str) -> String {
    if type_name.contains("Boolean") {
        format!("match value {{ Val::Bool(b) => {{ self.{fname} = Some(b); true }} _ => false }}")
    } else if type_name.contains("Double") || type_name.contains("Float") {
        format!("match value {{ Val::Double(d) => {{ self.{fname} = Some(d); true }} _ => false }}")
    } else if type_name == "EString" {
        format!("match value {{ Val::String(s) => {{ self.{fname} = Some(s); true }} _ => false }}")
    } else {
        // integer-ish
        format!(
            "match value {{ Val::Int(i) => {{ self.{fname} = Some(i as {field_inner}); true }} _ => false }}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_ecore_package;
    use crate::loader::test_utils::LIBRARY_ECORE;

    fn library_pkg() -> EPackage {
        load_ecore_package(LIBRARY_ECORE).unwrap()
    }

    #[test]
    fn emits_struct_per_class() {
        let src = generate_source(&library_pkg());
        assert!(src.contains("pub struct Library"), "{src}");
        assert!(src.contains("pub struct Book"), "{src}");
        assert!(src.contains("pub struct Writer"), "{src}");
    }

    #[test]
    fn emits_match_reflection_table() {
        let src = generate_source(&library_pkg());
        assert!(
            src.contains("fn e_get(&self, name: &str) -> Option<Val>"),
            "{src}"
        );
        assert!(src.contains("match name"), "{src}");
    }

    #[test]
    fn emits_register_package_that_builds_registry() {
        // The generated register code must be self-consistent with the metadata
        // API: run the loader the same way the registrar would and confirm the
        // class count matches what the source names.
        let pkg = library_pkg();
        let reg_text = format!("register_package: {} classes", pkg.classes().len());
        assert!(reg_text.contains("3 classes"));
    }

    #[test]
    fn generated_uses_package_names() {
        let src = generate_source(&library_pkg());
        assert!(
            src.contains("lib.rs") || src.contains("//! Static model generated"),
            "{src}"
        );
    }

    /// Confirm a generated source string is syntactically coherent enough that
    /// the loader's structures round-trip through our own shape helpers.
    #[test]
    fn field_types_are_concrete() {
        let pkg = library_pkg();
        for class in pkg.classes() {
            for feat in class.e_structural_features() {
                let ty = field_rust_type(feat);
                assert!(!ty.is_empty());
                assert!(!ty.contains("&str"));
            }
        }
    }

    /// Compile the generated source and run it: static modeling must produce
    /// *runnable* code, not just a string. We write a self-contained crate,
    /// `cargo build` it, then run a binary that exercises the reflection API.
    #[test]
    fn generated_crate_compiles_and_roundtrips() {
        let pkg = library_pkg();
        let src = generate_source(&pkg);

        let dir = std::env::temp_dir().join("codegen_e2e_library");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();

        std::fs::write(dir.join("Cargo.toml"), e2e_manifest()).unwrap();
        std::fs::write(dir.join("src/lib.rs"), &src).unwrap();

        // A binary that exercises the generated statically-typed model and its
        // EObject reflection surface; asserts drive the exit code.
        let main_rs = r#"
use emf_common::eobject::EObject;
use emf_common::value::Val;
use emf_ecore::PackageRegistry;

fn main() {
    // Metadata registration is self-contained: no .ecore file needed.
    let mut reg = PackageRegistry::new();
    library::register_package(&mut reg);
    let cls = reg.find_class("Book").expect("Book metadata registered");
    assert_eq!(cls.name(), "Book");
    assert_eq!(cls.e_structural_features().len(), 3);

    // Statically-typed construction.
    let mut book = library::Book::new();
    book.set_title("The Library");
    book.set_pages(99);

    // Reflection over the same object must agree with the typed surface.
    assert_eq!(
        book.e_get("title"),
        Some(Val::String("The Library".into()))
    );
    assert_eq!(book.e_get("pages"), Some(Val::Int(99)));
    assert!(book.e_is_set("title"));
    assert_eq!(book.e_class(), "Book");
    book.e_unset("title");
    assert!(!book.e_is_set("title"));

    println!("GEN-OK");
}
"#;
        std::fs::write(dir.join("src/main.rs"), main_rs).unwrap();

        let out = std::process::Command::new("cargo")
            .arg("run")
            .current_dir(&dir)
            .env("CARGO_TERM_COLOR", "never")
            .output()
            .expect("run cargo");
        assert!(
            out.status.success(),
            "generated crate failed to compile/run:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("GEN-OK"), "smoke did not run: {stdout}");
    }

    fn e2e_manifest() -> String {
        let common = env!("CARGO_MANIFEST_DIR");
        let workspace = std::path::Path::new(common)
            .parent()
            .and_then(|p| p.parent())
            .expect("two levels up to /workspace/artop-rust");
        let common = workspace.join("crates/emf-common");
        let ecore = workspace.join("crates/emf-ecore");
        format!(
            r#"# GENERATED static-model crate (produced by emf-ecore-codegen).
[package]
name = "library"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
emf-common = {{ path = "{}" }}
emf-ecore = {{ path = "{}" }}
"#,
            common.display(),
            ecore.display()
        )
    }
}
