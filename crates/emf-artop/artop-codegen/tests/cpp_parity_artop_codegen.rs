//! C++ parity tests for `artop-codegen`.
//!
//! Ports the three cases of the C++ `emf-artop-codegen/tests/test_main.cpp`
//! (`test_config_defaults`, `test_generate_on_small_ecore`,
//! `test_generate_on_autosar448`). As in C++, the large-model case is skipped
//! when the `.ecore` is not checked out.

use std::path::PathBuf;

use artop_codegen::{ArtopGenConfig, ArtopGenerator};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `crates/` — the workspace's crate root.
fn crates_root() -> PathBuf {
    manifest_dir().join("../..")
}

fn temp_out(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// 用例 1: ArtopGenConfig 默认值
#[test]
fn test_config_defaults() {
    let cfg = ArtopGenConfig::default();
    assert_eq!(cfg.version, "4.4.8");
    assert_eq!(cfg.release_id, "org.artop.aal.autosar448");
    assert_eq!(cfg.base_namespace_uri, "http://autosar.org/schema/r4.0");
    assert!(cfg.generate_resource);
    assert!(cfg.inject_root_extensions);
}

// 用例 2: 找一个小型 ecore 跑 codegen，确认输出文件
#[test]
fn test_generate_on_small_ecore() {
    let small = crates_root().join("emf-ecore-codegen/samples/library.ecore");
    if !small.exists() {
        println!("  [SKIP] library.ecore not found at {}", small.display());
        return;
    }
    let out = temp_out("artop-codegen-test-out");

    let cfg = ArtopGenConfig {
        input_ecore: small,
        out_dir: out.clone(),
        ..Default::default()
    };
    ArtopGenerator::new(cfg).generate_from_file().unwrap();

    // Base files (C++ `LibraryPackage.h` / `LibraryFactory.h`): the Rust base
    // emits one `lib.rs` carrying the package registrar and the classes.
    let lib = std::fs::read_to_string(out.join("src/lib.rs")).unwrap();
    assert!(lib.contains("pub fn register_package"), "{lib}");
    assert!(lib.contains("pub struct Library"), "{lib}");
    // ARTOP-specific files.
    assert!(out.join("src/resource.rs").exists());
    assert!(out.join("src/resource_factory.rs").exists());
    assert!(out.join("ARTOP_ROOT_EXTENSIONS.md").exists());

    // The resource glue aliases the runtime types under the package prefix.
    let resource = std::fs::read_to_string(out.join("src/resource.rs")).unwrap();
    assert!(resource.contains("pub type LibraryResource = AutosarXMLResource"));
    let factory = std::fs::read_to_string(out.join("src/resource_factory.rs")).unwrap();
    assert!(factory.contains("pub type LibraryResourceFactory = AutosarResourceFactory"));
    assert!(factory.contains("org.artop.aal.autosar448"));
}

// 用例 3: 加载 autosar448.ecore（大文件），验证跨包 / 规模不崩
#[test]
fn test_generate_on_autosar448() {
    let candidates = [
        // CI: the reference checkout lives at `<repo>/reference/artop-cpp`.
        manifest_dir().join("../../../../reference/artop-cpp/models/autosar448/autosar448.ecore"),
        PathBuf::from("/workspace/artop-cpp/models/autosar448/autosar448.ecore"),
    ];
    let Some(ecore) = candidates.iter().find(|p| p.exists()) else {
        println!("  [SKIP] autosar448.ecore not found");
        return;
    };
    let out = temp_out("artop-codegen-test-448");

    let cfg = ArtopGenConfig {
        input_ecore: ecore.clone(),
        out_dir: out.clone(),
        ..Default::default()
    };
    ArtopGenerator::new(cfg).generate_from_file().unwrap();

    // The root package of `autosar448.ecore` is `autosar40`, so the ARTOP glue
    // is emitted with the `Autosar40` prefix (C++ `Autosar40ResourceImpl.h`).
    let resource = std::fs::read_to_string(out.join("src/resource.rs")).unwrap();
    assert!(
        resource.contains("pub type Autosar40Resource = AutosarXMLResource"),
        "{resource}"
    );
    let factory = std::fs::read_to_string(out.join("src/resource_factory.rs")).unwrap();
    assert!(factory.contains("pub type Autosar40ResourceFactory = AutosarResourceFactory"));
    assert!(out.join("ARTOP_ROOT_EXTENSIONS.md").exists());
}
