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
use std::time::Instant;

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

/// Current process RSS in bytes, from /proc/self/status (Linux only).
fn rss_bytes() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            if let Some(kb) = rest.split_whitespace().next() {
                if let Ok(v) = kb.parse::<u64>() {
                    return v * 1024;
                }
            }
        }
    }
    0
}

/// Benchmark: time load (deserialize) and save (serialize) separately, mirroring
/// the C++ `ArxmlBenchmark`. Writes nothing to disk (save goes to a String).
fn bench(in_path: &str, iterations: usize) -> Result<(), String> {
    AutosarResourceFactory::register_default_autosar40_metamodel();
    let src = std::fs::read_to_string(in_path).map_err(|e| format!("reading `{in_path}`: {e}"))?;
    let file_size = src.len() as f64;
    println!("=== Rust artop-runtime Arxml Benchmark ===");
    println!("File: {in_path}");
    println!("Size: {:.1} MB ({} bytes)", file_size / 1048576.0, src.len());
    println!("Iterations: {iterations}\n");

    let mut load_ms = Vec::new();
    let mut save_ms = Vec::new();
    for i in 0..iterations {
        let rss_before = rss_bytes();

        let t0 = Instant::now();
        let mut res = AutosarXMLResource::new(
            Uri::parse(&format!("file:///{}", in_path)),
            ecore_package::global(),
        );
        res.load_from_string(&src)
            .map_err(|e| format!("loading `{in_path}`: {e}"))?;
        let load = t0.elapsed().as_secs_f64() * 1000.0;
        let roots = res.resource().contents().len();
        let rss_after_load = rss_bytes();

        let t1 = Instant::now();
        let out = res.save_to_string();
        let save = t1.elapsed().as_secs_f64() * 1000.0;
        let out_len = out.len();
        let rss_after = rss_bytes();
        drop(out);
        drop(res);

        load_ms.push(load);
        save_ms.push(save);
        println!(
            "Iter {}: load={:.0} ms, save={:.0} ms, total={:.0} ms | roots={} out={} bytes | rssBefore={:.1} MB rssAfterLoad={:.1} MB rssAfter={:.1} MB",
            i + 1, load, save, load + save, roots, out_len,
            rss_before as f64 / 1048576.0,
            rss_after_load as f64 / 1048576.0,
            rss_after as f64 / 1048576.0,
        );
    }

    let skip = if iterations > 1 { 1 } else { 0 };
    let n = (iterations - skip) as f64;
    let avg_load: f64 = load_ms[skip..].iter().sum::<f64>() / n;
    let avg_save: f64 = save_ms[skip..].iter().sum::<f64>() / n;
    println!("\n=== Summary (excl. warmup) ===");
    println!("Avg load: {:.0} ms ({:.1} MB/s)", avg_load, file_size / 1024.0 / avg_load);
    println!("Avg save: {:.0} ms ({:.1} MB/s)", avg_save, file_size / 1024.0 / avg_save);
    println!("Avg total: {:.0} ms", avg_load + avg_save);
    println!("=== DONE ===");
    Ok(())
}

fn usage() -> ExitCode {
    eprintln!("usage: arxml-roundtrip roundtrip <in.arxml> <out.arxml>");
    eprintln!("       arxml-roundtrip check     <in.arxml>");
    eprintln!("       arxml-roundtrip bench     <in.arxml> [iterations]");
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
        3 if args[1] == "bench" => bench(&args[2], 3),
        4 if args[1] == "roundtrip" => roundtrip(&args[2], &args[3]),
        4 if args[1] == "bench" => bench(&args[2], args[3].parse().unwrap_or(3)),
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
