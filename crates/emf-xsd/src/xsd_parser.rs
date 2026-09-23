//! XSD schema parser — turns a schema XML document into an [`XSDSchema`]
//! metamodel (aligned to Java `org.eclipse.xsd.util.XSDSchemaWalker` usage and
//! the C++ `emf-xsd` loader).
//!
//! Built on the dependency-free element tree parser of `emf-xmi`; it resolves
//! each element by its *local* name so the `xs:`, `xsd:` or any other prefix is
//! ignored. Occurrence bounds (`minOccurs` / `maxOccurs`) are parsed with
//! `maxOccurs="unbounded"` mapped to `-1`.

use emf_xmi::parser::{parse, XmlNode};

use crate::xsd_metamodel::{
    XSDAnnotation, XSDAttributeDeclaration, XSDComplexTypeDefinition, XSDCompositor,
    XSDElementDeclaration, XSDImport, XSDInclude, XSDRedefine, XSDSchema, XSDSimpleTypeDefinition,
    XsdCompositorKind, XsdFacet, XsdForm, XsdParticle, XsdParticleKind, XsdTypeRef, XsdUse,
};

/// Parse a schema document and return its top-level `XSDSchema`.
pub fn parse_schema(src: &str) -> Result<XSDSchema, String> {
    let roots = parse(src).map_err(|e| format!("XML parse error: {e}"))?;
    let schema_node = roots
        .into_iter()
        .find(|n| n.local == "schema")
        .ok_or_else(|| "no <schema> element found".to_string())?;
    Ok(schema_from_node(&schema_node))
}

fn attr<'a>(n: &'a XmlNode, name: &str) -> Option<&'a str> {
    n.attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn children<'a>(n: &'a XmlNode, local: &'a str) -> impl Iterator<Item = &'a XmlNode> + 'a {
    n.children
        .iter()
        .filter(move |c| c.local == local && !c.name.starts_with("/"))
}

fn first_child<'a>(n: &'a XmlNode, local: &str) -> Option<&'a XmlNode> {
    n.children.iter().find(|c| c.local == local)
}

fn bool_attr(n: &XmlNode, name: &str) -> bool {
    matches!(attr(n, name), Some("true") | Some("1"))
}

fn parse_max_occurs(s: Option<&str>) -> i32 {
    match s {
        Some("unbounded") => -1,
        Some(v) => v.parse::<i32>().unwrap_or(1),
        None => 1,
    }
}

fn parse_min_occurs(s: Option<&str>) -> i32 {
    s.and_then(|v| v.parse::<i32>().ok()).unwrap_or(1)
}

fn parse_form(s: Option<&str>) -> Option<XsdForm> {
    match s {
        Some("qualified") => Some(XsdForm::Qualified),
        Some("unqualified") => Some(XsdForm::Unqualified),
        _ => None,
    }
}

fn parse_annotation(n: &XmlNode) -> Option<XSDAnnotation> {
    let ann = children(n, "annotation").find(|_| true)?;
    let mut out = XSDAnnotation {
        source: attr(ann, "source").map(|s| s.to_string()),
        documentation: Vec::new(),
        appinfo: Vec::new(),
    };
    for d in children(ann, "documentation") {
        out.documentation.push(d.text.trim().to_string());
    }
    for a in children(ann, "appinfo") {
        out.appinfo.push((
            attr(a, "source").map(|s| s.to_string()),
            a.text.trim().to_string(),
        ));
    }
    Some(out)
}

fn parse_facets(restriction: &XmlNode) -> Vec<XsdFacet> {
    let mut facets = Vec::new();
    for c in &restriction.children {
        if c.name.starts_with("/") {
            continue;
        }
        let v = || attr(c, "value").unwrap_or("").to_string();
        let i = || {
            c.attr("value")
                .and_then(|x| x.parse::<i64>().ok())
                .unwrap_or(0)
        };
        match c.local.as_str() {
            "length" => facets.push(XsdFacet::Length(i())),
            "minLength" => facets.push(XsdFacet::MinLength(i())),
            "maxLength" => facets.push(XsdFacet::MaxLength(i())),
            "pattern" => facets.push(XsdFacet::Pattern(v())),
            "minInclusive" => facets.push(XsdFacet::MinInclusive(v())),
            "maxInclusive" => facets.push(XsdFacet::MaxInclusive(v())),
            "minExclusive" => facets.push(XsdFacet::MinExclusive(v())),
            "maxExclusive" => facets.push(XsdFacet::MaxExclusive(v())),
            "whiteSpace" => facets.push(XsdFacet::WhiteSpace(v())),
            "enumeration" => facets.push(XsdFacet::Enumeration(v())),
            _ => {}
        }
    }
    facets
}

fn parse_simple_type(n: &XmlNode) -> XSDSimpleTypeDefinition {
    let mut t = XSDSimpleTypeDefinition {
        name: attr(n, "name").unwrap_or("").to_string(),
        ..Default::default()
    };
    if let Some(restriction) = first_child(n, "restriction") {
        if let Some(base) = attr(restriction, "base") {
            t.base_name = Some(base.to_string());
            t.builtin = base.starts_with("xs:") || base.starts_with("xsd:");
        }
        let facets = parse_facets(restriction);
        for f in facets {
            if let XsdFacet::Enumeration(v) = &f {
                t.enumerations.push(v.clone());
            }
            t.facets.push(f);
        }
    } else if let Some(list) = first_child(n, "list") {
        t.base_name = attr(list, "itemType").map(|s| s.to_string());
        t.builtin = false;
    }
    t
}

fn parse_complex_type(n: &XmlNode) -> XSDComplexTypeDefinition {
    let mut t = XSDComplexTypeDefinition {
        name: attr(n, "name").unwrap_or("").to_string(),
        is_mixed: bool_attr(n, "mixed"),
        ..Default::default()
    };
    // simpleContent / complexContent may carry the base via restriction/extension.
    if let Some(content) =
        first_child(n, "complexContent").or_else(|| first_child(n, "simpleContent"))
    {
        if bool_attr(content, "mixed") {
            t.is_mixed = true;
        }
        // base may live on restriction OR extension inside the content.
        for kind in ["restriction", "extension"] {
            if let Some(r) = first_child(content, kind) {
                if let Some(base) = attr(r, "base") {
                    t.base_name = Some(base.to_string());
                }
                for a in children(r, "attribute") {
                    t.attributes.push(parse_attribute(a));
                }
                if let Some(any) = first_child(r, "anyAttribute") {
                    let _ = any;
                    t.has_any_attribute = true;
                }
                if let Some(seq) = children(r, "sequence").next() {
                    t.compositor = Some(parse_compositor(seq));
                } else if let Some(ch) = children(r, "choice").next() {
                    t.compositor = Some(parse_compositor(ch));
                } else if let Some(all) = children(r, "all").next() {
                    t.compositor = Some(parse_compositor(all));
                }
            }
        }
    } else {
        // Direct compositor on the complexType body.
        if let Some(seq) = children(n, "sequence").next() {
            t.compositor = Some(parse_compositor(seq));
        } else if let Some(ch) = children(n, "choice").next() {
            t.compositor = Some(parse_compositor(ch));
        } else if let Some(all) = children(n, "all").next() {
            t.compositor = Some(parse_compositor(all));
        }
        for a in children(n, "attribute") {
            t.attributes.push(parse_attribute(a));
        }
        if first_child(n, "anyAttribute").is_some() {
            t.has_any_attribute = true;
        }
    }
    t.annotation = parse_annotation(n);
    t
}

fn parse_compositor(n: &XmlNode) -> XSDCompositor {
    let kind = match n.local.as_str() {
        "choice" => XsdCompositorKind::Choice,
        "all" => XsdCompositorKind::All,
        _ => XsdCompositorKind::Sequence,
    };
    let mut comp = XSDCompositor::new(kind);
    for c in &n.children {
        if c.name.starts_with("/") {
            continue;
        }
        match c.local.as_str() {
            "element" => {
                let p = parse_element(c).particle();
                comp.particles.push(p);
            }
            "sequence" | "choice" | "all" => {
                let group = parse_compositor(c);
                comp.particles.push(XsdParticle::new(
                    XsdParticleKind::Group(group),
                    parse_min_occurs(attr(c, "minOccurs")),
                    parse_max_occurs(attr(c, "maxOccurs")),
                ));
            }
            "any" | "group" => {
                // opaque wildcard / named group reference; keep a placeholder element.
                comp.particles.push(XsdParticle::new(
                    XsdParticleKind::Group(XSDCompositor::new(kind)),
                    parse_min_occurs(attr(c, "minOccurs")),
                    parse_max_occurs(attr(c, "maxOccurs")),
                ));
            }
            _ => {}
        }
    }
    comp
}

fn parse_attribute(n: &XmlNode) -> XSDAttributeDeclaration {
    XSDAttributeDeclaration {
        name: attr(n, "name").unwrap_or("").to_string(),
        type_name: attr(n, "type").map(|s| s.to_string()),
        use_kind: match attr(n, "use") {
            Some("required") => XsdUse::Required,
            Some("prohibited") => XsdUse::Prohibited,
            _ => XsdUse::Optional,
        },
        default_value: attr(n, "default").map(|s| s.to_string()),
        fixed_value: attr(n, "fixed").map(|s| s.to_string()),
        form: parse_form(attr(n, "form")),
        annotation: parse_annotation(n),
    }
}

fn parse_element(n: &XmlNode) -> XSDElementDeclaration {
    let mut e = XSDElementDeclaration {
        name: attr(n, "name").unwrap_or("").to_string(),
        type_name: attr(n, "type").map(|s| s.to_string()),
        min_occurs: parse_min_occurs(attr(n, "minOccurs")),
        max_occurs: parse_max_occurs(attr(n, "maxOccurs")),
        is_abstract: bool_attr(n, "abstract"),
        is_nillable: bool_attr(n, "nillable"),
        substitution_group: attr(n, "substitutionGroup").map(|s| s.to_string()),
        default_value: attr(n, "default").map(|s| s.to_string()),
        fixed_value: attr(n, "fixed").map(|s| s.to_string()),
        type_definition: None,
        annotation: parse_annotation(n),
    };
    // Inline anonymous type definition.
    if let Some(tn) = first_child(n, "simpleType") {
        e.type_definition = Some(XsdTypeRef::Simple(parse_simple_type(tn)));
    } else if let Some(tn) = first_child(n, "complexType") {
        e.type_definition = Some(XsdTypeRef::Complex(parse_complex_type(tn)));
    }
    e
}

/// Convert a parsed `<schema>` node into an `XSDSchema`.
pub fn schema_from_node(node: &XmlNode) -> XSDSchema {
    let mut schema = XSDSchema {
        target_namespace: attr(node, "targetNamespace").map(|s| s.to_string()),
        element_form_default: parse_form(attr(node, "elementFormDefault"))
            .unwrap_or(XsdForm::Unqualified),
        attribute_form_default: parse_form(attr(node, "attributeFormDefault"))
            .unwrap_or(XsdForm::Unqualified),
        version: attr(node, "version").map(|s| s.to_string()),
        ..Default::default()
    };
    // 与 complexType parse_annotation(n) 一致：从容器节点收集 schema 级注解。
    // （此前在循环中把 annotation 元素本身传给 parse_annotation，因其子节点不含
    // annotation，恒返回 None，导致 schema 级注解被静默丢弃。）
    if let Some(a) = parse_annotation(node) {
        schema.annotations.push(a);
    }
    for c in &node.children {
        if c.name.starts_with("/") {
            continue;
        }
        match c.local.as_str() {
            "annotation" => {
                // annotation 元素本身已被上方 parse_annotation(node) 收集；
                // 此分支保留仅作兼容（对 annotation 元素调用本语义返回 None）。
            }
            "element" => schema.elements.push(parse_element(c)),
            "complexType" => schema
                .types
                .push(XsdTypeRef::Complex(parse_complex_type(c))),
            "simpleType" => schema.types.push(XsdTypeRef::Simple(parse_simple_type(c))),
            "attribute" => schema.attributes.push(parse_attribute(c)),
            "import" => schema.imports.push(XSDImport {
                namespace: attr(c, "namespace").map(|s| s.to_string()),
                schema_location: attr(c, "schemaLocation").map(|s| s.to_string()),
            }),
            "include" => schema.includes.push(XSDInclude {
                schema_location: attr(c, "schemaLocation").unwrap_or("").to_string(),
            }),
            "redefine" => schema.redefines.push(XSDRedefine {
                schema_location: attr(c, "schemaLocation").unwrap_or("").to_string(),
            }),
            _ => {}
        }
    }
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="http://example.com/ns"
           elementFormDefault="qualified">
  <xs:annotation>
    <xs:documentation>A sample automotive config schema.</xs:documentation>
  </xs:annotation>
  <xs:simpleType name="Color">
    <xs:restriction base="xs:string">
      <xs:enumeration value="red"/>
      <xs:enumeration value="blue"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:complexType name="RootType">
    <xs:sequence>
      <xs:element name="part" type="xs:int" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
    <xs:attribute name="id" type="xs:ID" use="required"/>
  </xs:complexType>
  <xs:element name="root" type="RootType"/>
  <xs:import namespace="http://other.example.com/ns" schemaLocation="other.xsd"/>
</xs:schema>"#;

    #[test]
    fn parse_basic_schema() {
        let schema = parse_schema(SAMPLE).expect("parses");
        assert_eq!(
            schema.target_namespace.as_deref(),
            Some("http://example.com/ns")
        );
        assert_eq!(schema.element_form_default, XsdForm::Qualified);
        assert_eq!(schema.imports.len(), 1);
        assert_eq!(schema.elements.len(), 1);

        let root = schema.element_by_name("root").expect("root element");
        assert_eq!(root.type_name.as_deref(), Some("RootType"));

        let color = schema.type_by_name("Color").expect("color type");
        match color {
            XsdTypeRef::Simple(t) => {
                assert_eq!(t.base_name.as_deref(), Some("xs:string"));
                assert_eq!(t.enumerations, vec!["red".to_string(), "blue".to_string()]);
            }
            _ => panic!("expected simple type"),
        }

        let rt = schema.type_by_name("RootType").expect("root type");
        match rt {
            XsdTypeRef::Complex(t) => {
                let seq = t.compositor.as_ref().expect("compositor");
                assert_eq!(seq.kind, XsdCompositorKind::Sequence);
                assert_eq!(seq.particles.len(), 1);
                assert!(seq.particles[0].is_unbounded());
                assert_eq!(t.attributes.len(), 1);
                assert_eq!(t.attributes[0].use_kind, XsdUse::Required);
            }
            _ => panic!("expected complex type"),
        }
    }

    #[test]
    fn unbounded_parses_to_minus_one() {
        assert_eq!(parse_max_occurs(Some("unbounded")), -1);
        assert_eq!(parse_max_occurs(None), 1);
        assert_eq!(parse_max_occurs(Some("3")), 3);
    }
}
