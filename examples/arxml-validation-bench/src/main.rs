//! `arxml-validation-bench` — model validation benchmark, mirroring the C++
//! `benchmark/cpp/ValidationBenchmark.cpp`.
//!
//!   arxml-validation-bench <in.arxml> [iterations]
//!
//! Per iteration measures:
//!   1. load                — ARXML deserialization
//!   2. batch validate      — `ValidationService` over the whole containment
//!                            tree with the EMF defaults + AUTOSAR + 49 ECUC
//!                            constraints, plus the model-level UUID sweep
//!   3. live attach         — `ValidationLiveAdapter::attach`
//!   4. live validateNow    — single-object incremental validation
//!
//! The first iteration is treated as warmup and excluded from the summary
//! (matching the C++ harness).

use std::process::ExitCode;
use std::time::Instant;

use artop_runtime::{AutosarResourceFactory, AutosarXMLResource};
use artop_validation::{
    register_autosar_constraints, register_ecuc_constraints, validate_uuid_uniqueness,
};
use emf_common::uri::Uri;
use emf_ecore::ecore_package;
use emf_validation::e_validator::EValidator;
use emf_validation::live_validator::ValidationLiveAdapter;
use emf_validation::validation_service::ValidationService;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("usage: arxml-validation-bench <in.arxml> [iterations]");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let iterations: usize = if args.len() == 3 {
        match args[2].parse() {
            Ok(n) if n >= 1 => n,
            _ => {
                eprintln!("iterations must be a positive integer");
                return ExitCode::from(2);
            }
        }
    } else {
        3
    };

    AutosarResourceFactory::register_default_autosar40_metamodel();

    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: reading `{path}`: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("=== Rust Validation Benchmark ===");
    println!("File: {path}");
    println!("Iterations: {iterations}");
    println!();

    let (mut sum_load, mut sum_vs, mut sum_attach, mut sum_live) = (0f64, 0f64, 0f64, 0f64);
    let mut diag_count = 0usize;

    for i in 0..iterations {
        // ---- 1. load ----
        let t = Instant::now();
        let mut res = AutosarXMLResource::new(
            Uri::parse(&format!("file:///{}", path)),
            ecore_package::global(),
        );
        if let Err(e) = res.load_from_string(&src) {
            eprintln!("ERROR: loading `{path}`: {e}");
            return ExitCode::FAILURE;
        }
        let load_ms = t.elapsed().as_secs_f64() * 1000.0;

        let contents = res.resource().contents();
        if contents.is_empty() {
            eprintln!("ERROR: `{path}`: no root element loaded");
            return ExitCode::FAILURE;
        }
        let root = contents[0].clone();
        let root_ref = root.borrow();

        // ---- 2. batch validate (EMF defaults + AUTOSAR + 49 ECUC + UUID sweep) ----
        // C++ `ValidationService`'s ctor auto-registers the generic EMF defaults
        // (`registerDefaultConstraints`); the Rust service starts empty and makes
        // that opt-in, so register them here for a like-for-like constraint set.
        let mut service = ValidationService::new();
        service.validator().register_default_constraints();
        register_autosar_constraints(service.validator());
        register_ecuc_constraints(service.validator());
        let t = Instant::now();
        let mut vs_diags = service.validate_all(&*root_ref);
        vs_diags.extend(validate_uuid_uniqueness(&*root_ref));
        let vs_ms = t.elapsed().as_secs_f64() * 1000.0;
        diag_count = vs_diags.len();
        drop(root_ref);

        // ---- 3. live attach ----
        let mut live = ValidationLiveAdapter::new(EValidator::new());
        let t = Instant::now();
        live.attach(root.clone());
        let attach_ms = t.elapsed().as_secs_f64() * 1000.0;

        // ---- 4. live validateNow (single object, incremental) ----
        let t = Instant::now();
        let _ = live.validate_now(&*root.borrow());
        let lv_ms = t.elapsed().as_secs_f64() * 1000.0;

        println!(
            "Iter {}: load={:.0} ms, batch={:.0} ms ({} diags), liveAttach={:.0} ms, liveValidate={:.0} ms",
            i + 1,
            load_ms,
            vs_ms,
            vs_diags.len(),
            attach_ms,
            lv_ms
        );

        if i > 0 {
            sum_load += load_ms;
            sum_vs += vs_ms;
            sum_attach += attach_ms;
            sum_live += lv_ms;
        }
    }

    let n = if iterations > 1 { iterations - 1 } else { 1 } as f64;
    println!();
    println!("=== Summary (excl. warmup) ===");
    println!("Avg load:              {:.0} ms", sum_load / n);
    println!("Avg batch validate:    {:.0} ms ({} diagnostics)", sum_vs / n, diag_count);
    println!("Avg live attach:       {:.0} ms", sum_attach / n);
    println!("Avg live validateNow:  {:.0} ms", sum_live / n);
    println!("=== DONE ===");
    ExitCode::SUCCESS
}