//! Example binary `arxml-validate`. Port of the same-named C++ example.
//! Currently a stub: demonstrates the `artop-common` diagnostic chain while the
//! ARXML validation passes are completed.
use artop_common::diagnostic::{Diagnostic, DiagnosticChain, Severity};

fn main() {
    let mut chain = DiagnosticChain::new();
    let msg = String::from("arxml validate");
    chain.add(Diagnostic::new(Severity::Info, "arxml-validate", 0, msg));
    eprintln!("{} diagnostics, worst = {:?}", chain.len(), chain.worst());
    assert_eq!(chain.worst(), Some(Severity::Info));
}
