//! arxml 互操作 harness 的 Rust 半侧。
//!
//!   cargo run -p artop-runtime --example arxml_roundtrip -- roundtrip <in.arxml> <out.arxml>
//!   cargo run -p artop-runtime --example arxml_roundtrip -- check     <in.arxml>

use artop_runtime::autosar_resource::AutosarXMLResource;
use artop_runtime::autosar_resource_factory::AutosarResourceFactory;
use emf_common::uri::Uri;
use emf_ecore::ecore_package;

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

fn roundtrip(in_path: &str, out_path: &str) -> Result<(), String> {
    let res = load(in_path)?;
    let out = res.save_to_string();
    std::fs::write(out_path, &out).map_err(|e| format!("writing `{out_path}`: {e}"))?;
    println!(
        "ROUNDTRIP-OK {out_path} ({} bytes in, {} bytes out)",
        std::fs::metadata(in_path).map(|m| m.len()).unwrap_or(0),
        out.len()
    );
    Ok(())
}

fn check(in_path: &str) -> Result<(), String> {
    let res = load(in_path)?;
    let contents = res.resource().contents();
    if contents.is_empty() {
        return Err("no root element loaded".to_string());
    }
    println!(
        "CHECK-OK {in_path} (root class {})",
        contents[0].borrow().e_class()
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.len() {
        3 if args[1] == "check" => check(&args[2]),
        4 if args[1] == "roundtrip" => roundtrip(&args[2], &args[3]),
        _ => {
            eprintln!("usage: arxml_roundtrip check <in.arxml>");
            eprintln!("       arxml_roundtrip roundtrip <in.arxml> <out.arxml>");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("INTEROP-FAIL: {e}");
        std::process::exit(1);
    }
}
