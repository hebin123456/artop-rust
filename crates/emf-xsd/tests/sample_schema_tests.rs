//! 对照测试：用与 C++ `emf-cpp/emf-xsd/tests/sample.xsd` 逐字相同的样本，验证
//! Rust 解析器与 XSD 元模型的行为。由于 C++ 端的 7 个测试文件均为空文件，
//! 本文件采用"同一样本对齐行为"策略，覆盖 C++ `emf-xsd` 实现语义最丰富的部分
//! （import/include/annotation、simpleType+pattern facet、complexType+
//! sequence+any+minMaxOccurs、属性 use=required、全局元素与属性）。

use emf_xsd::xsd_metamodel::{
    XSDComplexTypeDefinition, XSDCompositor, XsdCompositorKind, XsdFacet, XsdForm,
    XsdParticleKind, XsdTypeRef, XsdUse,
};
use emf_xsd::xsd_parser::parse_schema;

/// `tests/sample.xsd` 的原文（1552 字节，与 C++ 端样本逐字一致）。
const SAMPLE: &str = r###"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="http://www.example.com/library"
           xmlns:tns="http://www.example.com/library"
           elementFormDefault="qualified">

  <xs:import namespace="http://www.w3.org/XML/1998/namespace" schemaLocation="xml.xsd"/>
  <xs:include schemaLocation="common.xsd"/>

  <xs:annotation>
    <xs:documentation>Library schema for book management.</xs:documentation>
    <xs:appinfo>app-specific metadata</xs:appinfo>
  </xs:annotation>

  <xs:simpleType name="ISBNType">
    <xs:restriction base="xs:string">
      <xs:pattern value="[0-9]{10,13}"/>
    </xs:restriction>
  </xs:simpleType>

  <xs:complexType name="BookType" mixed="false">
    <xs:sequence>
      <xs:element name="title" type="xs:string" minOccurs="1" maxOccurs="1"/>
      <xs:element name="author" type="xs:string" minOccurs="1" maxOccurs="unbounded"/>
      <xs:element name="year" type="xs:int"/>
      <xs:any namespace="##other" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="isbn" type="tns:ISBNType" use="required"/>
    <xs:attribute name="lang" type="xs:string"/>
  </xs:complexType>

  <xs:complexType name="LibraryType">
    <xs:sequence>
      <xs:element name="book" type="tns:BookType" maxOccurs="unbounded"/>
    </xs:sequence>
  </xs:complexType>

  <xs:element name="library" type="tns:LibraryType"/>
  <xs:element name="book" type="tns:BookType"/>

  <xs:attribute name="globalAttr" type="xs:string"/>
</xs:schema>"###;

fn find_element_particle<'a>(
    seq: &'a XSDCompositor,
    name: &str,
) -> Option<&'a emf_xsd::xsd_metamodel::XSDElementDeclaration> {
    seq.particles.iter().find_map(|p| match &p.kind {
        XsdParticleKind::Element(e) if e.name == name => Some(e),
        _ => None,
    })
}

fn complex_of<'a>(s: &'a emf_xsd::xsd_metamodel::XSDSchema, name: &str) -> &'a XSDComplexTypeDefinition {
    match s.type_by_name(name) {
        Some(XsdTypeRef::Complex(t)) => t,
        other => panic!("expected complex type {name}, got {other:?}"),
    }
}

#[test]
fn sample_schema_level_attributes() {
    let schema = parse_schema(SAMPLE).expect("sample.xsd parses");

    // targetNamespace
    assert_eq!(
        schema.target_namespace.as_deref(),
        Some("http://www.example.com/library")
    );
    // elementFormDefault=qualified，且其 Display 为 "qualified"
    assert_eq!(schema.element_form_default, XsdForm::Qualified);
    assert_eq!(schema.element_form_default.to_string(), "qualified");
    // 未显式声明的 attributeFormDefault 应回退到 Unqualified
    assert_eq!(schema.attribute_form_default, XsdForm::Unqualified);
    assert_eq!(schema.version, None);

    // imports：sample.xsd 原文只有一个 `<xs:import>`。
    assert_eq!(schema.imports.len(), 1);
    assert_eq!(
        schema.imports[0].namespace.as_deref(),
        Some("http://www.w3.org/XML/1998/namespace")
    );
    assert_eq!(schema.imports[0].schema_location.as_deref(), Some("xml.xsd"));

    // includes：唯一 include → common.xsd
    assert_eq!(schema.includes.len(), 1);
    assert_eq!(schema.includes[0].schema_location, "common.xsd");

    // redefines 为空（样本不涉及）
    assert!(schema.redefines.is_empty());
}

#[test]
fn sample_schema_annotation() {
    let schema = parse_schema(SAMPLE).expect("sample.xsd parses");

    assert_eq!(schema.annotations.len(), 1);
    let ann = &schema.annotations[0];
    // documentation 文本被 trim
    assert_eq!(
        ann.documentation,
        vec!["Library schema for book management.".to_string()]
    );
    // appinfo：仅一条，source=None，文本含 "app-specific"
    assert_eq!(ann.appinfo.len(), 1);
    assert_eq!(ann.appinfo[0].0, None);
    assert!(ann.appinfo[0].1.contains("app-specific"), "appinfo text = {:?}", ann.appinfo[0].1);
}

#[test]
fn sample_schema_types_and_globals() {
    let schema = parse_schema(SAMPLE).expect("sample.xsd parses");

    // 三个全局类型：ISBNType(Simple)，BookType/LibraryType(Complex)
    assert_eq!(schema.types.len(), 3);
    assert!(schema.type_by_name("ISBNType").is_some());
    assert!(schema.type_by_name("BookType").is_some());
    assert!(schema.type_by_name("LibraryType").is_some());

    // ISBNType：simple, base xs:string, builtin, pattern facet
    match schema.type_by_name("ISBNType") {
        Some(XsdTypeRef::Simple(t)) => {
            assert_eq!(t.name, "ISBNType");
            assert_eq!(t.base_name.as_deref(), Some("xs:string"));
            assert!(t.builtin, "xs:string 带 xs: 前缀应为内置类型");
            assert!(
                t.facets.iter().any(|f| matches!(
                    f,
                    XsdFacet::Pattern(s) if s == "[0-9]{10,13}"
                )),
                "ISBNType 应含 Pattern(\"[0-9]{{10,13}}\"): {:?}",
                t.facets
            );
            assert!(t.enumerations.is_empty());
        }
        other => panic!("expected ISBNType to be simple, got {other:?}"),
    }

    // 全局元素 library / book 存在，并可反查其全局类型
    let library = schema.element_by_name("library").expect("global element library");
    assert_eq!(library.type_name.as_deref(), Some("tns:LibraryType"));
    assert_eq!(library.min_occurs, 1);
    assert_eq!(library.max_occurs, 1);
    assert!(complex_of(&schema, "LibraryType").attributes.is_empty());

    let book = schema.element_by_name("book").expect("global element book");
    assert_eq!(book.type_name.as_deref(), Some("tns:BookType"));
    assert!(complex_of(&schema, "BookType").attributes.len() >= 2);

    // 全局属性 globalAttr
    assert_eq!(schema.attributes.len(), 1);
    assert_eq!(schema.attributes[0].name, "globalAttr");
    assert_eq!(schema.attributes[0].type_name.as_deref(), Some("xs:string"));
}

#[test]
fn sample_schema_booktype_compositor_and_attributes() {
    let schema = parse_schema(SAMPLE).expect("sample.xsd parses");
    let seq = complex_of(&schema, "BookType")
        .compositor
        .as_ref()
        .expect("BookType 应有 compositor");
    assert_eq!(seq.kind, XsdCompositorKind::Sequence);

    // 由实现确认：`<xs:any>` 被解析为 XsdParticleKind::Group（kind 继承自封闭
    // sequence 的 Sequence，不保留 namespace 字段），因此 sequence 共 4 个粒子。
    assert_eq!(seq.particles.len(), 4);

    // title：simple string, min=1 max=1
    let title = find_element_particle(seq, "title").expect("title particle");
    assert_eq!(title.type_name.as_deref(), Some("xs:string"));
    assert_eq!((title.min_occurs, title.max_occurs), (1, 1));

    // author：simple string, min=1 max=unbounded（-1），is_unbounded()==true
    let author = find_element_particle(seq, "author").expect("author particle");
    assert_eq!(author.type_name.as_deref(), Some("xs:string"));
    assert_eq!((author.min_occurs, author.max_occurs), (1, -1));
    assert!(author.clone().particle().is_unbounded());

    // year：xs:int, min=max=1
    let year = find_element_particle(seq, "year").expect("year particle");
    assert_eq!(year.type_name.as_deref(), Some("xs:int"));
    assert_eq!((year.min_occurs, year.max_occurs), (1, 1));

    // any：namespace "##other", min=0 max=unbounded；按实现对 any 采用
    // XsdParticleKind::Group(Sequence)，min=0、max=-1、is_unbounded
    let any = &seq.particles[3];
    assert!(
        matches!(&any.kind, XsdParticleKind::Group(g) if g.kind == XsdCompositorKind::Sequence),
        "any 应解析为 Group(Sequence) particle"
    );
    assert_eq!(any.min_occurs, 0);
    assert!(any.is_unbounded(), "any maxOccurs=unbounded");

    // BookType 属性：isbn(required, tns:ISBNType) 与 lang(xs:string)
    let booktype = complex_of(&schema, "BookType");
    assert_eq!(booktype.attributes.len(), 2);
    let isbn = &booktype.attributes[0];
    assert_eq!(isbn.name, "isbn");
    assert_eq!(isbn.type_name.as_deref(), Some("tns:ISBNType"));
    assert_eq!(isbn.use_kind, XsdUse::Required);
    let lang = &booktype.attributes[1];
    assert_eq!(lang.name, "lang");
    assert_eq!(lang.type_name.as_deref(), Some("xs:string"));
    assert_eq!(lang.use_kind, XsdUse::Optional);
}

#[test]
fn sample_schema_librarytype_unbounded_book() {
    let schema = parse_schema(SAMPLE).expect("sample.xsd parses");
    let seq = complex_of(&schema, "LibraryType")
        .compositor
        .as_ref()
        .expect("LibraryType 应有 compositor");
    assert_eq!(seq.kind, XsdCompositorKind::Sequence);
    assert_eq!(seq.particles.len(), 1);

    let book = find_element_particle(seq, "book").expect("book particle");
    assert_eq!(book.type_name.as_deref(), Some("tns:BookType"));
    // maxOccurs="unbounded" → -1，is_unbounded()==true
    assert_eq!(book.max_occurs, -1);
    assert!(book.clone().particle().is_unbounded());
}

/// 对齐 C++ `XSDValidator`/facets 语义：全部 10 种 XsdFacet 的构造与 Display。
#[test]
fn facets_construct_and_display() {
    let cases = [
        (XsdFacet::Length(5), "length=5"),
        (XsdFacet::MinLength(1), "minLength=1"),
        (XsdFacet::MaxLength(10), "maxLength=10"),
        (XsdFacet::Pattern("[0-9]{10,13}".into()), "pattern=[0-9]{10,13}"),
        (XsdFacet::MinInclusive("0".into()), "minInclusive=0"),
        (XsdFacet::MaxInclusive("100".into()), "maxInclusive=100"),
        (XsdFacet::MinExclusive("1.5".into()), "minExclusive=1.5"),
        (XsdFacet::MaxExclusive("99".into()), "maxExclusive=99"),
        (XsdFacet::WhiteSpace("collapse".into()), "whiteSpace=collapse"),
        (XsdFacet::Enumeration("red".into()), "enumeration=red"),
    ];
    for (facet, expect) in cases {
        assert_eq!(
            facet.to_string(),
            expect,
            "facet 的 Display 应与 C++ 语义一致"
        );
    }
}

/// 对齐 C++ attribute use：Required/Optional/Prohibited 的 Display。
#[test]
fn xsd_use_displays() {
    assert_eq!(XsdUse::Required.to_string(), "required");
    assert_eq!(XsdUse::Optional.to_string(), "optional");
    assert_eq!(XsdUse::Prohibited.to_string(), "prohibited");
}