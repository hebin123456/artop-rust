//! ARTOP static-model generator (port of C++ `emf-artop/emf-artop-codegen`).
//!
//! This is the Rust counterpart of the C++ `ArtopCppGenerator`: it builds on the
//! generic [`emf_ecore_codegen`] generator (the C++ `CppGenerator` base) and adds
//! the ARTOP-specific steps on top:
//!
//! 1. [`ArtopGenerator::generate_from_package`] writes the base model crate, then
//! 2. emits the `<Pkg>Resource` / `<Pkg>ResourceFactory` glue (the Rust analogue
//!    of the C++ `<Pkg>ResourceImpl.h` / `<Pkg>ResourceFactoryImpl.h`), and
//! 3. records the root-extension injection plan as `ARTOP_ROOT_EXTENSIONS.md`.
//!
//! Beyond the per-class crate, [`registry_gen`] emits the *full* AUTOSAR static
//! registry (all `eSubpackages`, 2000+ classes, ARXML serialization metadata) as
//! plain data — the artifact the arxml layer reads/writes against, replacing the
//! out-of-band `tools/gen-autosar448-model.py` with the Rust code generator.
//!
//! ```no_run
//! use artop_codegen::{ArtopGenConfig, ArtopGenerator};
//! let mut config = ArtopGenConfig::default();
//! config.input_ecore = "autosar448.ecore".into();
//! config.out_dir = "/tmp/autosar448-model".into();
//! ArtopGenerator::new(config).generate_from_file()?;
//! # Ok::<(), String>(())
//! ```

pub mod generator;
pub mod registry_gen;

pub use generator::{ArtopGenConfig, ArtopGenerator};
pub use registry_gen::{generate_registry, generate_registry_from_files, RegistryStats};
