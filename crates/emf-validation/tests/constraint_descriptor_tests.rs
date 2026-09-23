//! Port of C++ `emf-validation/tests/ConstraintDescriptorTests.cpp`.
//!
//! Covers the `ConstraintDescriptor` defaults, setters and the
//! `ConstraintDescriptorParser::parseDescriptors` XML parsing.

use emf_validation::constraint::{ConstraintMode, Severity};
use emf_validation::constraint_descriptor::{ConstraintDescriptor, ConstraintDescriptorParser};

#[test]
fn defaults() {
    // ConstraintDescriptor_Defaults: severity WARNING, mode BATCH, code 0.
    let d = ConstraintDescriptor::default();
    assert_eq!(d.severity(), Severity::Warning);
    assert_eq!(d.mode(), ConstraintMode::Batch);
    assert_eq!(d.code(), 0);
}

#[test]
fn setters() {
    // ConstraintDescriptor_Setters / ConstraintDescriptor_SetSeverity.
    let mut d = ConstraintDescriptor::default();
    d.set_id("c1");
    d.set_name("Name");
    d.set_message("msg");
    d.set_severity(Severity::Error);
    assert_eq!(d.id(), "c1");
    assert_eq!(d.name(), "Name");
    assert_eq!(d.message(), "msg");
    assert_eq!(d.severity(), Severity::Error);
}

#[test]
fn parse_descriptors() {
    // ConstraintDescriptor_ParseDescriptors: two self-closing constraints.
    let xml = "<constraints>\
               <constraint id=\"a\" name=\"A\" message=\"ma\" severity=\"error\"/>\
               <constraint id=\"b\" name=\"B\" message=\"mb\" severity=\"warning\"/>\
               </constraints>";
    let v = ConstraintDescriptorParser::parse_descriptors(xml);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].id(), "a");
    assert_eq!(v[1].id(), "b");
    assert_eq!(v[0].severity(), Severity::Error);
    assert_eq!(v[1].severity(), Severity::Warning);
}