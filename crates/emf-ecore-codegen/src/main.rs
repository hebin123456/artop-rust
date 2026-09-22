//! Command-line static-model generator: `emf-ecore-codegen <model.ecore>`.
//!
//! Feeds an `.ecore` document to `GenModel` and writes a self-contained Rust
//! crate to an output directory (default: the package name, in the CWD).
//!
//! ```ignore
//! emf-ecore-codegen library.ecore              # -> ./library/{Cargo.toml,src/lib.rs}
//! emf-ecore-codegen model.ecore --out gen/model
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use emf_ecore_codegen::{CrateSpec, GenModel};

const USAGE: &str = "\
emf-ecore-codegen <model.ecore> [options]

Generate a self-contained Rust crate from an Ecore metamodel.

ARGS:
    <model.ecore>    Path to the .ecore model document.

OPTIONS:
    --out <DIR>      Output directory (default: ./<package-name>).
    --name <NAME>    Generated crate name (default: the package name).
    --common <PATH>  Root of the emf-common crate (default: sibling).
    --ecore <PATH>   Root of the emf-ecore crate (default: sibling).
    -h, --help       Show this help and exit.

The generated crate compiles and runs independently of the source .ecore.
";

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut spec = CrateSpec::default();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--out" => {
                out = args.next().map(PathBuf::from);
            }
            "--name" => {
                spec.package_name = args.next().unwrap_or_default();
            }
            "--common" => {
                spec.emf_common = args.next().map(PathBuf::from).unwrap_or_default();
            }
            "--ecore" => {
                spec.emf_ecore = args.next().map(PathBuf::from).unwrap_or_default();
            }
            _ if arg.starts_with('-') => {
                eprintln!("unknown option: {arg}\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
            _ => {
                if input.is_none() {
                    let p = PathBuf::from(arg);
                    if spec.package_name.is_empty() {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            spec.package_name = stem.to_string();
                        }
                    }
                    input = Some(p);
                }
            }
        }
    }

    let Some(input) = input else {
        eprintln!("missing <model.ecore> argument.\n\n{USAGE}");
        return ExitCode::FAILURE;
    };
    if !input.exists() {
        eprintln!("no such file: {}", input.display());
        return ExitCode::FAILURE;
    }

    let model = match GenModel::load_path(&input) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("failed to load `{}`: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };
    if spec.package_name.is_empty() {
        spec.package_name = model.package_name().to_string();
    }
    let out = out.unwrap_or_else(|| PathBuf::from(&spec.package_name));

    match model.generate_crate(&out, &spec) {
        Ok(()) => {
            println!(
                "generated crate `{}` at {} ({} classes)",
                spec.package_name,
                out.display(),
                model.package().classes().len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("generation failed: {e}");
            ExitCode::FAILURE
        }
    }
}
