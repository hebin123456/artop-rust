//! `XSDSchema` metamodel — a plain-data model of an XML Schema document
//! (aligned to Java `org.eclipse.xsd` and C++ `emf-xsd/xsd_metamodel.h`).
//!
//! This is a general-purpose, artop-agnostic XSD metamodel: it mirrors the
//! structural elements of `xsd.xsd` (schema, complexType, simpleType,
//! element, attribute, annotation, compositors, facets, import/include/redefine)
//! as ordinary Rust types with a fluent builder API. The parser in
//! [`crate::xsd_parser`] populates it from a schema document.

use std::fmt;

/// How an attribute declaration is used (XML Schema `use`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XsdUse {
    /// `use="optional"` (default) — may be absent.
    Optional,
    /// Not permitted in the instance document.
    Prohibited,
    /// `use="required"` — must be present.
    Required,
}

impl Default for XsdUse {
    fn default() -> Self {
        XsdUse::Optional
    }
}

impl fmt::Display for XsdUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            XsdUse::Optional => "optional",
            XsdUse::Prohibited => "prohibited",
            XsdUse::Required => "required",
        })
    }
}

/// Element / attribute form (namespace qualification) controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XsdForm {
    /// Namespace-qualified local elements/attributes.
    #[default]
    Qualified,
    /// Unqualified local elements/attributes.
    Unqualified,
}

impl fmt::Display for XsdForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            XsdForm::Qualified => "qualified",
            XsdForm::Unqualified => "unqualified",
        })
    }
}

/// A compositor (model group): `xs:sequence` / `xs:choice` / `xs:all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XsdCompositorKind {
    Sequence,
    Choice,
    All,
}

impl fmt::Display for XsdCompositorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            XsdCompositorKind::Sequence => "sequence",
            XsdCompositorKind::Choice => "choice",
            XsdCompositorKind::All => "all",
        })
    }
}

/// Annotation (metadata) on any XSD schema component.
#[derive(Debug, Clone, Default)]
pub struct XSDAnnotation {
    /// `source` document (e.g. a URL) of the annotation.
    pub source: Option<String>,
    /// Text content of `<xs:documentation>` children.
    pub documentation: Vec<String>,
    /// Source attributes of `<xs:appinfo>` children.
    pub appinfo: Vec<(Option<String>, String)>,
}

impl XSDAnnotation {
    /// New empty annotation.
    pub fn new() -> Self {
        Self::default()
    }
}

/// A `particle` inside a model group: a repeated element declaration or a
/// nested group.
#[derive(Debug, Clone)]
pub enum XsdParticleKind {
    /// An element declaration reference/content.
    Element(XSDElementDeclaration),
    /// A nested model group (compositor).
    Group(XSDCompositor),
}

/// A particle with occurrence bounds.
#[derive(Debug, Clone)]
pub struct XsdParticle {
    /// What this particle repeats.
    pub kind: XsdParticleKind,
    /// `minOccurs` (default 1; 0 = optional).
    pub min_occurs: i32,
    /// `maxOccurs` (default 1; -1 = unbounded).
    pub max_occurs: i32,
}

impl XsdParticle {
    /// A particle wrapping `kind` with the given occurrence bounds.
    pub fn new(kind: XsdParticleKind, min_occurs: i32, max_occurs: i32) -> Self {
        Self {
            kind,
            min_occurs,
            max_occurs,
        }
    }
    /// Whether unbounded (`maxOccurs="unbounded"`).
    pub fn is_unbounded(&self) -> bool {
        self.max_occurs < 0
    }
}

/// A model group: `sequence` / `choice` / `all` with child particles.
#[derive(Debug, Clone)]
pub struct XSDCompositor {
    /// Group kind.
    pub kind: XsdCompositorKind,
    /// Child particles, in order.
    pub particles: Vec<XsdParticle>,
}

impl XSDCompositor {
    /// New empty compositor of `kind`.
    pub fn new(kind: XsdCompositorKind) -> Self {
        Self {
            kind,
            particles: Vec::new(),
        }
    }
    /// Append a particle.
    pub fn add_particle(&mut self, p: XsdParticle) -> &mut Self {
        self.particles.push(p);
        self
    }
}

/// A facet of a simple type restriction.
#[derive(Debug, Clone, PartialEq)]
pub enum XsdFacet {
    /// `length="N"`.
    Length(i64),
    /// `minLength="N"`.
    MinLength(i64),
    /// `maxLength="N"`.
    MaxLength(i64),
    /// `pattern="regex"`.
    Pattern(String),
    /// `minInclusive="v"`.
    MinInclusive(String),
    /// `maxInclusive="v"`.
    MaxInclusive(String),
    /// `minExclusive="v"`.
    MinExclusive(String),
    /// `maxExclusive="v"`.
    MaxExclusive(String),
    /// `whiteSpace="preserve|replace|collapse"`.
    WhiteSpace(String),
    /// A single `enumeration="v"` value.
    Enumeration(String),
}

impl fmt::Display for XsdFacet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XsdFacet::Length(v) => write!(f, "length={v}"),
            XsdFacet::MinLength(v) => write!(f, "minLength={v}"),
            XsdFacet::MaxLength(v) => write!(f, "maxLength={v}"),
            XsdFacet::Pattern(v) => write!(f, "pattern={v}"),
            XsdFacet::MinInclusive(v) => write!(f, "minInclusive={v}"),
            XsdFacet::MaxInclusive(v) => write!(f, "maxInclusive={v}"),
            XsdFacet::MinExclusive(v) => write!(f, "minExclusive={v}"),
            XsdFacet::MaxExclusive(v) => write!(f, "maxExclusive={v}"),
            XsdFacet::WhiteSpace(v) => write!(f, "whiteSpace={v}"),
            XsdFacet::Enumeration(v) => write!(f, "enumeration={v}"),
        }
    }
}

/// A simple type definition (a restriction/union/list of a base type).
#[derive(Debug, Clone, Default)]
pub struct XSDSimpleTypeDefinition {
    /// Type name (`name`), empty when anonymous.
    pub name: String,
    /// Qualified base type, e.g. `"xs:string"` or a global type name.
    pub base_name: Option<String>,
    /// Whether a built-in XSD type.
    pub builtin: bool,
    /// Restriction or list members, in order.
    pub facets: Vec<XsdFacet>,
    /// Enumeration members (convenience view of [`XsdFacet::Enumeration`]).
    pub enumerations: Vec<String>,
}

impl XSDSimpleTypeDefinition {
    /// New named simple type.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    /// Set the qualified base type name.
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base_name = Some(base.into());
        self
    }
    /// Append a facet.
    pub fn with_facet(mut self, f: XsdFacet) -> Self {
        match &f {
            XsdFacet::Enumeration(v) => self.enumerations.push(v.clone()),
            _ => {}
        }
        self.facets.push(f);
        self
    }
}

/// An attribute declaration (`xs:attribute`).
#[derive(Debug, Clone, Default)]
pub struct XSDAttributeDeclaration {
    /// Attribute name.
    pub name: String,
    /// Qualified type, e.g. `"xs:NMTOKEN"`.
    pub type_name: Option<String>,
    /// Whether the attribute is banned / optional / required.
    pub use_kind: XsdUse,
    /// `default` value (implies implicit use).
    pub default_value: Option<String>,
    /// `fixed` value.
    pub fixed_value: Option<String>,
    /// `form` qualification, if declared.
    pub form: Option<XsdForm>,
    /// Attached annotation (e.g. ARXML `xs:documentation`).
    pub annotation: Option<XSDAnnotation>,
}

impl XSDAttributeDeclaration {
    /// New empty attribute declaration.
    pub fn new() -> Self {
        Self::default()
    }
    /// Attribute name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// An element declaration (`xs:element`).
#[derive(Debug, Clone, Default)]
pub struct XSDElementDeclaration {
    /// Element name.
    pub name: String,
    /// Qualified type of the element content.
    pub type_name: Option<String>,
    /// `minOccurs` (default 1).
    pub min_occurs: i32,
    /// `maxOccurs` (default 1; -1 = unbounded).
    pub max_occurs: i32,
    /// `abstract="true"` — cannot appear directly.
    pub is_abstract: bool,
    /// `nillable="true"` — allow `xsi:nil`.
    pub is_nillable: bool,
    /// `substitutionGroup` qualified name.
    pub substitution_group: Option<String>,
    /// `default` value for simple content.
    pub default_value: Option<String>,
    /// `fixed` value.
    pub fixed_value: Option<String>,
    /// Optional in-place complex/simple type (`name` empty).
    pub type_definition: Option<XsdTypeRef>,
    /// Attached annotation.
    pub annotation: Option<XSDAnnotation>,
}

impl XSDElementDeclaration {
    /// A repeatable element wrapper (particle).
    pub fn particle(self) -> XsdParticle {
        let min = self.min_occurs;
        let max = self.max_occurs;
        XsdParticle::new(XsdParticleKind::Element(self), min, max)
    }
}

/// Reference to any type: either a named type or a built-in data type name.
#[derive(Debug, Clone)]
pub enum XsdTypeRef {
    /// A global or local (anonymous) simple type.
    Simple(XSDSimpleTypeDefinition),
    /// A global or local (anonymous) complex type.
    Complex(XSDComplexTypeDefinition),
    /// A bare qualified name (e.g. `"xs:string"`) with no local definition.
    Named(String),
}

impl XsdTypeRef {
    /// The human-readable type name (best effort).
    pub fn display_name(&self) -> &str {
        match self {
            XsdTypeRef::Simple(t) => {
                if t.name.is_empty() {
                    t.base_name.as_deref().unwrap_or("(anonymous simple)")
                } else {
                    &t.name
                }
            }
            XsdTypeRef::Complex(t) => {
                if t.name.is_empty() {
                    "(anonymous complex)"
                } else {
                    &t.name
                }
            }
            XsdTypeRef::Named(n) => n,
        }
    }
}

/// A complex type definition (`xs:complexType`).
#[derive(Debug, Clone, Default)]
pub struct XSDComplexTypeDefinition {
    /// Type name, empty when anonymous.
    pub name: String,
    /// Qualified base type (`base="..."`) if any.
    pub base_name: Option<String>,
    /// Whether mixed content is allowed.
    pub is_mixed: bool,
    /// The top-level model group (sequence/choice/all), if any.
    pub compositor: Option<XSDCompositor>,
    /// Direct attribute declarations.
    pub attributes: Vec<XSDAttributeDeclaration>,
    /// The `anyAttribute` allow flag.
    pub has_any_attribute: bool,
    /// Attached annotation.
    pub annotation: Option<XSDAnnotation>,
}

impl XSDComplexTypeDefinition {
    /// New named complex type.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    /// Set the top-level compositor.
    pub fn set_compositor(&mut self, c: XSDCompositor) -> &mut Self {
        self.compositor = Some(c);
        self
    }
    /// Append an attribute declaration.
    pub fn add_attribute(&mut self, a: XSDAttributeDeclaration) -> &mut Self {
        self.attributes.push(a);
        self
    }
}

/// `xs:import` — pulls in a schema from another namespace.
#[derive(Debug, Clone, Default)]
pub struct XSDImport {
    /// Imported namespace URI.
    pub namespace: Option<String>,
    /// `schemaLocation`, when given.
    pub schema_location: Option<String>,
}

/// `xs:include` — includes another schema in the same namespace.
#[derive(Debug, Clone, Default)]
pub struct XSDInclude {
    /// `schemaLocation`.
    pub schema_location: String,
}

/// `xs:redefine` — redefines components from an included schema.
#[derive(Debug, Clone, Default)]
pub struct XSDRedefine {
    /// `schemaLocation`.
    pub schema_location: String,
}

/// The top-level `xs:schema` model.
#[derive(Debug, Clone, Default)]
pub struct XSDSchema {
    /// `targetNamespace`.
    pub target_namespace: Option<String>,
    /// `elementFormDefault`.
    pub element_form_default: XsdForm,
    /// `attributeFormDefault`.
    pub attribute_form_default: XsdForm,
    /// `version`.
    pub version: Option<String>,
    /// Top-level element declarations.
    pub elements: Vec<XSDElementDeclaration>,
    /// Top-level type definitions (global), in document order.
    pub types: Vec<XsdTypeRef>,
    /// Top-level attribute declarations.
    pub attributes: Vec<XSDAttributeDeclaration>,
    /// `xs:import` directives.
    pub imports: Vec<XSDImport>,
    /// `xs:include` directives.
    pub includes: Vec<XSDInclude>,
    /// `xs:redefine` directives.
    pub redefines: Vec<XSDRedefine>,
    /// Schema-level annotations.
    pub annotations: Vec<XSDAnnotation>,
}

impl XSDSchema {
    /// New empty schema.
    pub fn new() -> Self {
        Self::default()
    }
    /// Look up a global complex/simple type by name.
    pub fn type_by_name(&self, name: &str) -> Option<&XsdTypeRef> {
        self.types.iter().find(|t| t.display_name() == name)
    }
    /// Look up a top-level element by name.
    pub fn element_by_name(&self, name: &str) -> Option<&XSDElementDeclaration> {
        self.elements.iter().find(|e| e.name == name)
    }
    /// Append a top-level type.
    pub fn add_type(&mut self, t: XsdTypeRef) -> &mut Self {
        self.types.push(t);
        self
    }
    /// Append a top-level element declaration.
    pub fn add_element(&mut self, e: XSDElementDeclaration) -> &mut Self {
        self.elements.push(e);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_pattern_builds() {
        let mut schema = XSDSchema::new();
        schema.target_namespace = Some("http://example.com/ns".into());

        let mut root = XSDComplexTypeDefinition::named("RootType");
        root.add_attribute(XSDAttributeDeclaration {
            name: "id".into(),
            type_name: Some("xs:ID".into()),
            use_kind: XsdUse::Required,
            ..Default::default()
        });
        let mut seq = XSDCompositor::new(XsdCompositorKind::Sequence);
        seq.add_particle(
            XSDElementDeclaration {
                max_occurs: -1,
                ..Default::default()
            }
            .particle(),
        );

        let mut root_el = XSDElementDeclaration::default();
        root_el.name = "root".into();
        root_el.type_name = Some("RootType".into());
        schema.add_element(root_el);
        schema.add_type(XsdTypeRef::Complex(root));

        assert_eq!(schema.elements.len(), 1);
        assert_eq!(schema.types.len(), 1);
        assert_eq!(
            schema
                .type_by_name("RootType")
                .map(XsdTypeRef::display_name),
            Some("RootType")
        );
    }

    #[test]
    fn facets_collect_enumerations() {
        let t = XSDSimpleTypeDefinition::named("Color")
            .with_base("xs:string")
            .with_facet(XsdFacet::Enumeration("red".into()))
            .with_facet(XsdFacet::Enumeration("blue".into()))
            .with_facet(XsdFacet::Pattern("[0-9]+".into()));
        assert_eq!(t.enumerations, vec!["red", "blue"]);
        assert_eq!(t.facets.len(), 3);
    }

    #[test]
    fn display_forms() {
        assert_eq!(XsdUse::Required.to_string(), "required");
        assert_eq!(XsdForm::Qualified.to_string(), "qualified");
    }
}
