//! `arxml-validate` — load an AUTOSAR ARXML file and run the AUTOSAR business
//! constraints over it (port of the C++ `arxml_validate` example).
//!
//!   arxml-validate <in.arxml>
//!
//! Loads through the same static-model [`artop_runtime`] reader as
//! `arxml-roundtrip`, then runs the batch constraints registered by
//! `artop_validation` (`emf-artop-validation`) over the whole containment tree.
//! Exits non-zero when any `Error` severity diagnostic is produced.

use std::process::ExitCode;

use artop_runtime::{AutosarResourceFactory, AutosarXMLResource};
use artop_validation::{register_autosar_constraints, register_ecuc_constraints};
use emf_common::diagnostic::Severity;
use emf_common::uri::Uri;
use emf_ecore::ecore_package;
use emf_validation::validation_service::ValidationService;

fn load(path: &str) -> Result<AutosarXMLResource, String> {
    AutosarResourceFactory::register_default_autosar40_metamodel();
    let src = std::fs::read_to_string(path).map_err(|e| format!("reading `{path}`: {e}"))?;
    let mut res = AutosarXMLResource::new(
        Uri::parse(&format!("file:///{}", path)),
        ecore_package::global(),
    );
    res.load_from_string(&src)
        .map_err(|e| format!("loading `{path}`: {e}"))?;
    Ok(res)
}

fn validate(path: &str) -> Result<usize, String> {
    let res = load(path)?;
    let contents = res.resource().contents();
    if contents.is_empty() {
        return Err(format!("`{path}`: no root element loaded"));
    }

    let mut service = ValidationService::new();
    register_autosar_constraints(service.validator());
    register_ecuc_constraints(service.validator());

    let diagnostics = service.validate_all(&*contents[0].borrow());
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity() == Severity::Error)
        .count();
    for d in &diagnostics {
        println!("{d}");
    }
    println!(
        "VALIDATE-OK {path} ({} diagnostic(s), {errors} error(s))",
        diagnostics.len()
    );
    Ok(errors)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("arxml-validate {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.len() != 2 {
        eprintln!("usage: arxml-validate <in.arxml>");
        eprintln!("       arxml-validate --version");
        return ExitCode::from(2);
    }
    match validate(&args[1]) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(n) => {
            eprintln!("ERROR: {n} validation error(s)");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}
