//! Integration tests for the public [`GenModel`] static-modeling API.
//!
//! These exercise the API as a downstream crate would: load a real `.ecore`
//! from disk, generate a self-contained crate, and *compile+run* it against a
//! fresh commandline invocation — without the original `.ecore` at runtime.

use std::path::{Path, PathBuf};

use emf_ecore_codegen::{CrateSpec, GenModel};

fn sample_ecore() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/library.ecore")
}

fn spec() -> CrateSpec {
    CrateSpec::default()
}

#[test]
fn loads_real_ecore_file() {
    let model = GenModel::load_path(sample_ecore()).unwrap();
    assert_eq!(model.package_name(), "library");
    let pkg = model.package();
    assert_eq!(pkg.classes().len(), 3);
    for name in ["Library", "Book", "Writer"] {
        assert!(pkg.find_class(name).is_some(), "missing class {name}");
    }
}

#[test]
fn generates_standalone_crate_that_builds() {
    let dir = std::env::temp_dir().join("codegen_it_build_library");
    let _ = std::fs::remove_dir_all(&dir);

    let model = GenModel::load_path(sample_ecore()).unwrap();
    model
        .generate_crate(
            &dir,
            &CrateSpec {
                package_name: "library_model".into(),
                ..spec()
            },
        )
        .unwrap();

    assert!(dir.join("Cargo.toml").exists());
    assert!(dir.join("src/lib.rs").exists());

    let out = std::process::Command::new("cargo")
        .arg("build")
        .current_dir(&dir)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("run cargo build on generated crate");
    assert!(
        out.status.success(),
        "generated crate failed to build:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn generated_crate_usable_from_binary() {
    let dir = std::env::temp_dir().join("codegen_it_run_library");
    let _ = std::fs::remove_dir_all(&dir);

    GenModel::load_path(sample_ecore())
        .unwrap()
        .generate_crate(
            &dir,
            &CrateSpec {
                package_name: "library_model".into(),
                ..spec()
            },
        )
        .unwrap();

    // A consumer binary that depends on the generated crate and exercises both
    // the typed surface and the EObject reflection table.
    std::fs::write(
        dir.join("src/main.rs"),
        r#"
use emf_common::eobject::EObject;
use emf_common::value::Val;
use emf_ecore::PackageRegistry;

fn main() {
    let mut reg = PackageRegistry::new();
    library_model::register_package(&mut reg);
    let book_cls = reg.find_class("Book").expect("Book metadata registered");
    assert_eq!(book_cls.e_structural_features().len(), 3);

    let mut book = library_model::Book::new();
    book.set_title("The Library");
    book.set_pages(99);

    assert_eq!(book.e_get("title"), Some(Val::String("The Library".into())));
    assert_eq!(book.e_get("pages"), Some(Val::Int(99)));
    assert!(book.e_is_set("title"));
    assert_eq!(book.e_class(), "Book");
    book.e_unset("title");
    assert!(!book.e_is_set("title"));

    println!("IT-OK");
}
"#,
    )
    .unwrap();

    let out = std::process::Command::new("cargo")
        .arg("run")
        .current_dir(&dir)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("run consumer binary");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "consumer binary failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("IT-OK"), "smoke did not run: {stdout}");
}
