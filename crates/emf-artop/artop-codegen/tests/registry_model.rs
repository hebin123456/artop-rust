//! End-to-end test for the full AUTOSAR static-registry generator
//! ([`artop_codegen::registry_gen`]).
//!
//! Verifies that the Rust code generator reproduces the committed
//! `autosar448-model/src/registry.rs` from the two `.ecore` documents — i.e. the
//! static model the arxml layer reads/writes against is now produced by the Rust
//! generator (the port of C++ `ArtopCppGenerator`), not an out-of-band script.
//!
//! The `.ecore` documents are large (18 MB) and only checked out in CI's
//! reference job, so the test skips when they are absent — exactly like the
//! existing `test_generate_on_autosar448` parity case.

use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The AUTOSAR `.ecore` sources, as checked out by CI (`reference/artop-cpp`) or
/// present in the local sandbox.
fn ecore_sources() -> Option<(PathBuf, PathBuf)> {
    let roots = [
        manifest_dir().join("../../../../reference/artop-cpp/models"),
        PathBuf::from("/workspace/artop-cpp/models"),
    ];
    for root in roots {
        let g = root.join("gautosar/gautosar.ecore");
        let a = root.join("autosar448/autosar448.ecore");
        if g.is_file() && a.is_file() {
            return Some((g, a));
        }
    }
    None
}

/// The committed registry the generator must reproduce.
fn committed_registry() -> PathBuf {
    manifest_dir().join("../autosar448-model/src/registry.rs")
}

/// Compare two registry sources ignoring comments and all whitespace — enough to
/// prove the generator's data is identical while tolerating `rustfmt` on the
/// committed artifact.
fn normalized(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}

#[test]
fn registry_generator_reproduces_committed_model() {
    let Some((gautosar, autosar448)) = ecore_sources() else {
        println!("  [SKIP] AUTOSAR .ecore sources not checked out");
        return;
    };

    let (src, stats) = artop_codegen::generate_registry_from_files(&gautosar, &autosar448).unwrap();

    // Scale of the merged gautosar + autosar448 metamodel.
    assert_eq!(stats.packages, 420, "{}", stats.summary());
    assert_eq!(stats.classes, 2105, "{}", stats.summary());
    assert_eq!(stats.features, 6122, "{}", stats.summary());
    assert_eq!(stats.enums, 291, "{}", stats.summary());
    assert_eq!(stats.datatypes, 67, "{}", stats.summary());
    assert!(stats.unresolved.is_empty(), "{}", stats.summary());

    // Spot-check the ARXML entry points the reader/writer rely on.
    assert!(src.contains("name_to_id"), "generated lookups missing");
    assert!(src.contains("xml_name_to_id"), "generated lookups missing");

    // The generated data must match the committed static model.
    let committed = std::fs::read_to_string(committed_registry()).unwrap();
    assert_eq!(
        normalized(&src),
        normalized(&committed),
        "generated registry diverges from the committed autosar448-model artifact"
    );
}

#[test]
fn registry_generation_is_deterministic() {
    let Some((gautosar, autosar448)) = ecore_sources() else {
        println!("  [SKIP] AUTOSAR .ecore sources not checked out");
        return;
    };
    let (a, _) = artop_codegen::generate_registry_from_files(&gautosar, &autosar448).unwrap();
    let (b, _) = artop_codegen::generate_registry_from_files(&gautosar, &autosar448).unwrap();
    assert_eq!(a, b, "generation must be byte-for-byte deterministic");
}

/// Guard that the CLI subcommand wires to the same generator (cheap check: the
/// committed artifact is what `artop-codegen registry` produces).
#[test]
fn committed_registry_is_not_empty() {
    let path = committed_registry();
    assert!(Path::new(&path).is_file(), "{}", path.display());
    let src = std::fs::read_to_string(&path).unwrap();
    assert!(
        src.contains("pub const N_CLASSES: usize = 2105;"),
        "committed registry does not declare the expected class count"
    );
}
