//! Command-line entry point.
//!
//! Port of the C++ `emf-artop-codegen` `main.cpp`:
//!
//! ```text
//! artop-codegen <ecore> <out-dir> [--version=X.Y.Z] [--release-id=ID]
//!               [--namespace=URI] [--no-resource] [--no-extensions]
//! artop-codegen registry <gautosar.ecore> <autosar448.ecore> <out/registry.rs>
//! ```
//!
//! The `registry` subcommand emits the full AUTOSAR static registry (all
//! `eSubpackages`, ARXML serialization metadata) that the arxml layer reads /
//! writes against — the Rust replacement for `tools/gen-autosar448-model.py`.
//!
//! The C++ tool also accepts a `.genmodel`; the Rust port's `GenModel` loads
//! `.ecore` documents only, so a `.genmodel` input is rejected with a pointer to
//! the underlying `.ecore`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use artop_codegen::{generate_registry_from_files, ArtopGenConfig, ArtopGenerator};

const USAGE: &str = "\
artop-codegen <ecore> <out-dir> [options]
artop-codegen registry <gautosar.ecore> <autosar448.ecore> <out.rs>

ARGS:
    <ecore>            Path to the .ecore metamodel document.
    <out-dir>          Output directory for the generated crate.

SUBCOMMANDS:
    registry           Emit the full AUTOSAR static registry (all subpackages +
                       ARXML metadata) from the two .ecore documents.

OPTIONS:
    --version=X.Y.Z    AUTOSAR version (default 4.4.8).
    --release-id=ID    Release id (default org.artop.aal.autosar448).
    --namespace=URI    Base namespace (default http://autosar.org/schema/r4.0).
    --no-resource      Skip the Resource/ResourceFactory glue.
    --no-extensions    Skip the root-extensions injection record.
    -h, --help         Show this help and exit.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.first().map(String::as_str) == Some("registry") {
        return run_registry(&args[1..]);
    }
    if args.len() < 2 {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }

    let input = PathBuf::from(&args[0]);
    let mut cfg = ArtopGenConfig {
        input_ecore: input.clone(),
        out_dir: PathBuf::from(&args[1]),
        ..Default::default()
    };

    for arg in &args[2..] {
        if let Some(v) = arg.strip_prefix("--version=") {
            cfg.version = v.to_string();
        } else if let Some(v) = arg.strip_prefix("--release-id=") {
            cfg.release_id = v.to_string();
        } else if let Some(v) = arg.strip_prefix("--namespace=") {
            cfg.base_namespace_uri = v.to_string();
        } else if arg == "--no-resource" {
            cfg.generate_resource = false;
        } else if arg == "--no-extensions" {
            cfg.inject_root_extensions = false;
        } else {
            eprintln!("unknown option: {arg}");
            return ExitCode::FAILURE;
        }
    }
    // C++ `main` recomputes the schema location from the (possibly overridden)
    // namespace.
    cfg.schema_location = format!("{} AUTOSAR_4-4-8.xsd", cfg.base_namespace_uri);

    if !input.is_file() {
        eprintln!("[artop-codegen] no such file: {}", input.display());
        return ExitCode::FAILURE;
    }
    if input.extension().and_then(|e| e.to_str()) == Some("genmodel") {
        eprintln!(
            "[artop-codegen] .genmodel input is not supported by the Rust port; \
             pass the underlying .ecore instead"
        );
        return ExitCode::FAILURE;
    }

    println!("[artop-codegen] input : {}", input.display());
    println!("[artop-codegen] output: {}", cfg.out_dir.display());
    println!("[artop-codegen] ver   : {}", cfg.version);

    match ArtopGenerator::new(cfg).generate_from_file() {
        Ok(()) => {
            println!("[artop-codegen] done.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("[artop-codegen] generation failed: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `artop-codegen registry <gautosar.ecore> <autosar448.ecore> <out.rs>`.
fn run_registry(args: &[String]) -> ExitCode {
    if args.len() != 3 {
        eprintln!("usage: artop-codegen registry <gautosar.ecore> <autosar448.ecore> <out.rs>");
        return ExitCode::FAILURE;
    }
    let gautosar = Path::new(&args[0]);
    let autosar448 = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    for p in [gautosar, autosar448] {
        if !p.is_file() {
            eprintln!("[artop-codegen] no such file: {}", p.display());
            return ExitCode::FAILURE;
        }
    }

    println!(
        "[artop-codegen] registry: {} + {}",
        gautosar.display(),
        autosar448.display()
    );
    match generate_registry_from_files(gautosar, autosar448) {
        Ok((src, stats)) => {
            if let Some(parent) = out.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("[artop-codegen] creating `{}`: {e}", parent.display());
                        return ExitCode::FAILURE;
                    }
                }
            }
            if let Err(e) = std::fs::write(out, src) {
                eprintln!("[artop-codegen] writing `{}`: {e}", out.display());
                return ExitCode::FAILURE;
            }
            println!("[artop-codegen] wrote {}", out.display());
            println!("[artop-codegen] stats: {}", stats.summary());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("[artop-codegen] registry generation failed: {e}");
            ExitCode::FAILURE
        }
    }
}
