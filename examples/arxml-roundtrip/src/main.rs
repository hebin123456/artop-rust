//! Example binary `arxml-roundtrip`. Port of the same-named C++ example.
//! Currently a stub: demonstrates the `emf-common` diagnostic API while the
//! XMI/ARXML reader-writer pipeline is completed.
use emf_common::diagnostic::{Diagnostic, Severity};

fn main() {
    let msg = String::from("arxml roundtrip");
    let d = Diagnostic::new(Severity::Info, "arxml-roundtrip", 0, msg);
    eprintln!("{d}");
    assert_ne!(d.severity(), Severity::Error);
}
