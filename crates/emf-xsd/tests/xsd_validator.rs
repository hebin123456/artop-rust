//! Behavioural tests for [`emf_xsd::validator::XSDValidator`].
//!
//! The C++ `emf-xsd` test files are empty, so these tests pin the validator's
//! behaviour directly: a self-contained schema exercises every diagnostic code
//! (`root_not_found`, `parse_error`, `element_in_simple_type`,
//! `extra_children`, `unexpected_child`, `choice_mismatch`, `all_mismatch`,
//! `minLength`/`maxLength`/`length`, `pattern`, `enumeration`,
//! `minInclusive`/`maxInclusive`/`minExclusive`/`maxExclusive`, `required_attr`)
//! plus the parser/matcher helpers.

use emf_xsd::validator::{XSDValidator, XSDValidatorOptions};
use emf_xsd::xsd_parser::parse_schema;

const SCHEMA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="urn:t" xmlns:tns="urn:t"
           elementFormDefault="qualified">
  <xs:simpleType name="Code">
    <xs:restriction base="xs:string">
      <xs:minLength value="3"/>
      <xs:maxLength value="3"/>
      <xs:pattern value="[0-9]{3}"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="Color">
    <xs:restriction base="xs:string">
      <xs:enumeration value="red"/>
      <xs:enumeration value="blue"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="Qty">
    <xs:restriction base="xs:int">
      <xs:minInclusive value="1"/>
      <xs:maxInclusive value="9"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="StrictQty">
    <xs:restriction base="xs:int">
      <xs:minExclusive value="0"/>
      <xs:maxExclusive value="10"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:complexType name="ItemType">
    <xs:sequence>
      <xs:element name="name" type="xs:string"/>
      <xs:element name="tag" type="xs:string" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="id" type="xs:string" use="required"/>
    <xs:attribute name="note" type="xs:string"/>
  </xs:complexType>
  <xs:complexType name="RootType">
    <xs:sequence>
      <xs:element name="item" type="tns:ItemType" maxOccurs="unbounded"/>
    </xs:sequence>
  </xs:complexType>
  <xs:complexType name="EmptyType"/>
  <xs:complexType name="PickType">
    <xs:choice>
      <xs:element name="cat" type="xs:string"/>
      <xs:element name="dog" type="xs:string"/>
    </xs:choice>
  </xs:complexType>
  <xs:element name="code" type="tns:Code"/>
  <xs:element name="color" type="tns:Color"/>
  <xs:element name="qty" type="tns:Qty"/>
  <xs:element name="strictqty" type="tns:StrictQty"/>
  <xs:element name="empty" type="tns:EmptyType"/>
  <xs:element name="pick" type="tns:PickType"/>
  <xs:element name="root" type="tns:RootType"/>
</xs:schema>"#;

fn schema() -> emf_xsd::XSDSchema {
    parse_schema(SCHEMA).expect("schema parses")
}

fn codes(diags: &[emf_xsd::XSDDiagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.as_str()).collect()
}

fn validate(xml: &str) -> Vec<emf_xsd::XSDDiagnostic> {
    XSDValidator::new().validate(&schema(), xml)
}

#[test]
fn valid_document_has_no_diagnostics() {
    let xml = r#"<root>
      <item id="a"><name>n1</name><tag>t1</tag><tag>t2</tag></item>
      <item id="b"><name>n2</name></item>
    </root>"#;
    assert!(validate(xml).is_empty(), "got {:?}", validate(xml));
}

#[test]
fn root_not_found() {
    let diags = validate("<nope/>");
    assert_eq!(codes(&diags), vec!["root_not_found"]);
    assert_eq!(diags[0].element_qname.as_deref(), Some("nope"));
}

#[test]
fn parse_error_reported() {
    let diags = validate("<root><item></root>");
    assert_eq!(codes(&diags), vec!["parse_error"]);
}

#[test]
fn element_of_simple_type_must_not_have_children() {
    let diags = validate("<code><x>1</x></code>");
    assert_eq!(codes(&diags), vec!["element_in_simple_type"]);
}

#[test]
fn length_and_pattern_facets() {
    // Exactly 3 digits: valid.
    assert!(validate("<code>123</code>").is_empty());

    // Too short: minLength + pattern.
    let short = validate("<code>12</code>");
    assert!(codes(&short).contains(&"minLength"), "{:?}", codes(&short));
    assert!(codes(&short).contains(&"pattern"), "{:?}", codes(&short));

    // Too long: maxLength + pattern.
    let long = validate("<code>1234</code>");
    assert!(codes(&long).contains(&"maxLength"), "{:?}", codes(&long));
    assert!(codes(&long).contains(&"pattern"), "{:?}", codes(&long));
}

#[test]
fn enumeration_facet() {
    assert!(validate("<color>red</color>").is_empty());
    let bad = validate("<color>green</color>");
    assert_eq!(codes(&bad), vec!["enumeration"]);
}

#[test]
fn inclusive_and_exclusive_numeric_facets() {
    assert!(validate("<qty>5</qty>").is_empty());
    assert_eq!(codes(&validate("<qty>0</qty>")), vec!["minInclusive"]);
    assert_eq!(codes(&validate("<qty>10</qty>")), vec!["maxInclusive"]);

    assert!(validate("<strictqty>5</strictqty>").is_empty());
    assert_eq!(
        codes(&validate("<strictqty>0</strictqty>")),
        vec!["minExclusive"]
    );
    assert_eq!(
        codes(&validate("<strictqty>10</strictqty>")),
        vec!["maxExclusive"]
    );
}

#[test]
fn empty_complex_type_rejects_children() {
    assert!(validate("<empty/>").is_empty());
    assert_eq!(
        codes(&validate("<empty><x>1</x></empty>")),
        vec!["extra_children"]
    );
}

#[test]
fn sequence_content_model_mismatch() {
    // `bogus` is not in ItemType's sequence.
    let diags = validate(r#"<root><item id="a"><name>n1</name><bogus>x</bogus></item></root>"#);
    assert_eq!(codes(&diags), vec!["unexpected_child"]);
}

#[test]
fn choice_content_model() {
    assert!(validate("<pick><cat>c</cat></pick>").is_empty());
    let diags = validate("<pick><fish>f</fish></pick>");
    assert_eq!(codes(&diags), vec!["choice_mismatch"]);
}

#[test]
fn required_attribute_enforced() {
    // `id` is required on ItemType.
    let diags = validate(r#"<root><item><name>n1</name></item></root>"#);
    assert_eq!(codes(&diags), vec!["required_attr"]);
    assert_eq!(diags[0].attribute_qname.as_deref(), Some("id"));
    assert_eq!(diags[0].element_qname.as_deref(), Some("item"));

    // Optional `note` absent is fine.
    assert!(validate(r#"<root><item id="z"><name>n1</name></item></root>"#).is_empty());
}

#[test]
fn options_disable_facet_checks() {
    let opts = XSDValidatorOptions {
        validate_facets: false,
        ..XSDValidatorOptions::default()
    };
    let validator = XSDValidator::with_options(opts);
    assert!(validator.validate(&schema(), "<code>12</code>").is_empty());
}

#[test]
fn options_disable_required_attributes() {
    let opts = XSDValidatorOptions {
        validate_required_attributes: false,
        ..XSDValidatorOptions::default()
    };
    let validator = XSDValidator::with_options(opts);
    assert!(validator
        .validate(&schema(), r#"<root><item><name>n1</name></item></root>"#)
        .is_empty());
}

#[test]
fn validate_preparsed_root_node() {
    let root = XSDValidator::parse_xml("<code>123</code>").expect("parses");
    let diags = XSDValidator::new().validate_node(&schema(), &root);
    assert!(diags.is_empty(), "{:?}", codes(&diags));
}

#[test]
fn parse_xml_rejects_empty_document() {
    assert!(XSDValidator::parse_xml("   ").is_err());
}
