//! Example binary `arxml-roundtrip` (skeleton). Port of the same-named C++ example.
use artop_common::diagnostic::Severity;

fn main() {
    let msg = String::from("arxml roundtrip");
    artop_common::diag::report(Severity::Info, &msg);
}
