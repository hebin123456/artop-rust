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
//! ```no_run
//! use artop_codegen::{ArtopGenConfig, ArtopGenerator};
//! let mut config = ArtopGenConfig::default();
//! config.input_ecore = "autosar448.ecore".into();
//! config.out_dir = "/tmp/autosar448-model".into();
//! ArtopGenerator::new(config).generate_from_file()?;
//! # Ok::<(), String>(())
//! ```

pub mod generator;

pub use generator::{ArtopGenConfig, ArtopGenerator};
