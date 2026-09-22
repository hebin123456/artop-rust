//! Top-level static-modeling entry point: [`GenModel`].
//!
//! This is the public face of the crate — the Rust analog of C++
//! `emf-ecore-codegen`'s `GenModelLoader` + `GenModel` combo. It chains the
//! two phases of the pipeline behind one object:
//!
//! 1. loading an `.ecore` document into an [`emf_ecore::EPackage`]
//!    ([`loader::load_ecore_package`]), and
//! 2. emitting a self-contained Rust crate worth of statically-typed source
//!    ([`generator::generate_source`]) that compiles without the `.ecore`.
//!
//! ```no_run
//! use emf_ecore_codegen::GenModel;
//! let model = GenModel::load(include_str!("../samples/library.ecore"))?;
//! model.generate_crate("/tmp/library_model", &Default::default())?;
//! # Ok::<(), String>(())
//! ```
//!
//! [`loader::load_ecore_package`]: crate::loader::load_ecore_package
//! [`generator::generate_source`]: crate::generator::generate_source

use std::path::{Path, PathBuf};

use emf_ecore::{EPackage, PackageRegistry};

use crate::loader::load_ecore_package;
use crate::typing::package_module_name;

/// A loaded, code-generatable Ecore model (one [`EPackage`]).
#[derive(Debug, Clone)]
pub struct GenModel {
    pkg: EPackage,
}

/// Build settings for emitting a generated crate to disk.
///
/// The only thing the generator cannot infer on its own is *where* the
/// runtime dependencies (`emf-common`, `emf-ecore`) live, because the crate is
/// self-contained by design. [`Default`] resolves them as the sibling crates
/// next to this one (workspace layout) — override when targeting a different
/// tree.
#[derive(Debug, Clone)]
pub struct CrateSpec {
    /// Name of the generated `[package]`. Defaults to the model's module-safe
    /// package name when empty.
    pub package_name: String,
    /// Filesystem path to the `emf-common` crate (its `Cargo.toml`).
    pub emf_common: PathBuf,
    /// Filesystem path to the `emf-ecore` crate (its `Cargo.toml`).
    pub emf_ecore: PathBuf,
}

impl Default for CrateSpec {
    fn default() -> Self {
        // This crate lives at <workspace>/crates/emf-ecore-codegen; the runtime
        // deps are the sibling crates emf-common and emf-ecore.
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let crates_root = here.parent().unwrap_or(here);
        Self {
            package_name: String::new(),
            emf_common: crates_root.join("emf-common"),
            emf_ecore: crates_root.join("emf-ecore"),
        }
    }
}

impl GenModel {
    /// Load an `.ecore` document from a string into an owned [`EPackage`].
    pub fn load(src: &str) -> Result<Self, String> {
        Ok(Self {
            pkg: load_ecore_package(src)?,
        })
    }

    /// Load a `.ecore` file from disk.
    pub fn load_path<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();
        let src = std::fs::read_to_string(path)
            .map_err(|e| format!("reading `{}`: {e}", path.display()))?;
        Self::load(&src)
    }

    /// The loaded package metadata (classes, features, enums, datatypes).
    pub fn package(&self) -> &EPackage {
        &self.pkg
    }

    /// The model/package name as declared in the `.ecore`.
    pub fn package_name(&self) -> &str {
        self.pkg.name()
    }

    /// Render the whole model as a single idiomatic `lib.rs` source string.
    pub fn generate_source(&self) -> String {
        crate::generator::generate_source(&self.pkg)
    }

    /// Generate and write a complete, self-contained crate to `out_dir`.
    ///
    /// Writes `Cargo.toml` and `src/lib.rs` (a `[workspace]`-root crate with
    /// explicit path dependencies on the runtime crates), so the output can be
    /// built and run without the source `.ecore`.
    pub fn generate_crate<P: AsRef<Path>>(
        &self,
        out_dir: P,
        spec: &CrateSpec,
    ) -> Result<(), String> {
        let out_dir = out_dir.as_ref();
        let name = if spec.package_name.is_empty() {
            package_module_name(self.pkg.name())
        } else {
            spec.package_name.clone()
        };

        let manifest = standalone_manifest(&name, &spec.emf_common, &spec.emf_ecore);
        let src = self.generate_source();

        let src_dir = out_dir.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| format!("creating `{}`: {e}", src_dir.display()))?;
        std::fs::write(out_dir.join("Cargo.toml"), manifest)
            .map_err(|e| format!("writing manifest: {e}"))?;
        std::fs::write(src_dir.join("lib.rs"), src).map_err(|e| format!("writing lib.rs: {e}"))?;
        Ok(())
    }

    /// Register this model's metadata into a registry (for reflection).
    pub fn register(&self, reg: &mut PackageRegistry) {
        reg.register(emf_ecore::make_package_ref(self.pkg.clone()));
    }
}

fn standalone_manifest(name: &str, common: &Path, ecore: &Path) -> String {
    format!(
        r#"# GENERATED static-model crate (produced by emf-ecore-codegen).
# Compiled here so the model is usable without the source .ecore file.
[package]
name = "{name}"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
emf-common = {{ path = "{common}" }}
emf-ecore = {{ path = "{ecore}" }}
"#,
        common = common.display(),
        ecore = ecore.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GenModel {
        GenModel::load(include_str!("../samples/library.ecore")).unwrap()
    }

    #[test]
    fn loads_from_path_and_string() {
        let by_path = GenModel::load_path(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/library.ecore"),
        )
        .unwrap();
        assert_eq!(by_path.package_name(), "library");
        assert_eq!(sample().package_name(), "library");
    }

    #[test]
    fn default_spec_resolves_sibling_deps() {
        let spec = CrateSpec::default();
        assert!(spec.emf_common.is_absolute());
        // TOML path values are valid on both separators.
        assert!(standalone_manifest("x", &spec.emf_common, &spec.emf_ecore).contains('\"'));
    }

    #[test]
    fn generates_self_contained_crate() {
        let dir = std::env::temp_dir().join("genmodel_crate_library");
        let _ = std::fs::remove_dir_all(&dir);
        sample()
            .generate_crate(&dir, &CrateSpec::default())
            .unwrap();

        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("[workspace]"), "{cargo}");
        assert!(cargo.contains("name = \"library\""), "{cargo}");
        let lib = std::fs::read_to_string(dir.join("src/lib.rs")).unwrap();
        assert!(lib.contains("pub struct Library"), "{lib}");
    }

    #[test]
    fn generates_every_class_and_registers() {
        let model = sample();
        let mut reg = PackageRegistry::new();
        model.register(&mut reg);
        for name in ["Library", "Book", "Writer"] {
            assert_eq!(
                reg.find_class(name).unwrap().name(),
                name,
                "class {name} registered"
            );
        }
    }
}
