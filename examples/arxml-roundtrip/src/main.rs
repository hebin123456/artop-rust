//! `arxml-roundtrip` — read and write AUTOSAR ARXML files with the generated
//! static model (port of the C++ `emf-artop-runtime` round-trip example).
//!
//!   arxml-roundtrip roundtrip <in.arxml> <out.arxml>   # load, then save
//!   arxml-roundtrip check     <in.arxml>               # load and report the root
//!   arxml-roundtrip --version
//!
//! The reader/writer are the [`artop_runtime`] arxml loader/saver, which resolve
//! every element through the static `autosar448-model` registry — so this binary
//! is the "直接调用读写 arxml" entry point shipped in the release archives.

use std::process::ExitCode;

use artop_runtime::{AutosarResourceFactory, AutosarXMLResource};
use emf_common::uri::Uri;
use emf_ecore::ecore_package;

/// Load an arxml document into an AUTOSAR resource using the static model.
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

/// Load then save — the round-trip the C++/Java serializers are compared against.
fn roundtrip(in_path: &str, out_path: &str) -> Result<(), String> {
    let res = load(in_path)?;
    let out = res.save_to_string();
    std::fs::write(out_path, &out).map_err(|e| format!("writing `{out_path}`: {e}"))?;
    let in_len = std::fs::metadata(in_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "ROUNDTRIP-OK {out_path} ({in_len} bytes in, {} bytes out)",
        out.len()
    );
    Ok(())
}

/// Load and report the root element / package count.
fn check(in_path: &str) -> Result<(), String> {
    let res = load(in_path)?;
    let contents = res.resource().contents();
    if contents.is_empty() {
        return Err(format!("`{in_path}`: no root element loaded"));
    }
    let root_class = contents[0].borrow().e_class().to_string();
    println!(
        "CHECK-OK {in_path} (root <{}>, {} root object(s))",
        root_class,
        contents.len()
    );
    Ok(())
}

fn usage() -> ExitCode {
    eprintln!("usage: arxml-roundtrip roundtrip <in.arxml> <out.arxml>");
    eprintln!("       arxml-roundtrip check     <in.arxml>");
    eprintln!("       arxml-roundtrip --version");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("arxml-roundtrip {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let result = match args.len() {
        3 if args[1] == "check" => check(&args[2]),
        4 if args[1] == "roundtrip" => roundtrip(&args[2], &args[3]),
        _ => return usage(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}
