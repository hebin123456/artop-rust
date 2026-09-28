//! `XSDValidator` — validate an XML instance document against an [`XSDSchema`]
//! (port of C++ `emf-xsd/src/XSDValidator.cpp`, aligned to Java
//! `org.eclipse.xsd.validation` semantics).
//!
//! The C++ reference ships an empty `XSDValidator.h` and a simplified
//! `XSDValidator.cpp`, and its test files are empty. This port keeps the same
//! public shape (`validate` / `parseXML` / `findGlobalElement` /
//! `validateElement` / `validateSimpleType` / `validateComplexType` /
//! `validateRequiredAttributes` and the diagnostic codes) while implementing
//! the Java-aligned behaviour where the C++ sketch was incomplete:
//!
//! - the content model honours `minOccurs` / `maxOccurs` (greedy + backtracking)
//!   instead of the C++ fixed one-child-per-particle loop;
//! - required attributes are driven by the declaration's `use="required"`
//!   (the Rust metamodel has no `scope`; C++ approximated with `scope==GLOBAL`);
//! - `pattern` facets use the crate-local [`crate::pattern`] engine in place of
//!   `std::regex`.
//!
//! The XML element tree is obtained from the `emf-xmi` element parser rather
//! than re-implementing the C++ hand-rolled reader.

use crate::pattern::regex_full_match;
use crate::xsd_metamodel::{
    XSDComplexTypeDefinition, XSDElementDeclaration, XSDSchema, XSDSimpleTypeDefinition,
    XsdCompositorKind, XsdFacet, XsdParticle, XsdParticleKind, XsdTypeRef, XsdUse,
};
use emf_common::diagnostic::Severity;
use emf_xmi::parser::{parse, XmlNode};

/// A single validation diagnostic (C++ `XSDDiagnostic`).
#[derive(Debug, Clone, PartialEq)]
pub struct XSDDiagnostic {
    /// Severity (always [`Severity::Error`] for the checks performed here).
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `"pattern"`, `"required_attr"`.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Qualified name of the offending element, if known.
    pub element_qname: Option<String>,
    /// Qualified name of the offending attribute, if known.
    pub attribute_qname: Option<String>,
}

impl XSDDiagnostic {
    /// New diagnostic with the given severity, code and message.
    pub fn new(severity: Severity, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            element_qname: None,
            attribute_qname: None,
        }
    }

    /// New error diagnostic.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    /// Record the offending element's qualified name.
    pub fn set_element_qname(&mut self, qname: impl Into<String>) {
        self.element_qname = Some(qname.into());
    }

    /// Record the offending attribute's qualified name.
    pub fn set_attribute_qname(&mut self, qname: impl Into<String>) {
        self.attribute_qname = Some(qname.into());
    }
}

/// Which checks [`XSDValidator`] performs (C++ `options_`).
#[derive(Debug, Clone, Copy)]
pub struct XSDValidatorOptions {
    /// Validate simple-type `facets` (length/pattern/enumeration/bounds).
    pub validate_facets: bool,
    /// Validate complex-type content models (sequence/choice/all).
    pub validate_content_model: bool,
    /// Validate that `use="required"` attributes are present.
    pub validate_required_attributes: bool,
}

impl Default for XSDValidatorOptions {
    fn default() -> Self {
        Self {
            validate_facets: true,
            validate_content_model: true,
            validate_required_attributes: true,
        }
    }
}

/// Validates XML instance documents against an [`XSDSchema`].
#[derive(Debug, Clone, Default)]
pub struct XSDValidator {
    options: XSDValidatorOptions,
}

impl XSDValidator {
    /// New validator with all checks enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// New validator with explicit options.
    pub fn with_options(options: XSDValidatorOptions) -> Self {
        Self { options }
    }

    /// The active options.
    pub fn options(&self) -> XSDValidatorOptions {
        self.options
    }

    /// Parse an XML document into its root element (C++ `parseXML`).
    pub fn parse_xml(xml: &str) -> Result<XmlNode, String> {
        let roots = parse(xml)?;
        roots
            .into_iter()
            .next()
            .ok_or_else(|| "empty XML: no root element".to_string())
    }

    /// Validate the XML document text against `schema`.
    pub fn validate(&self, schema: &XSDSchema, xml: &str) -> Vec<XSDDiagnostic> {
        let mut diags = Vec::new();
        let root = match Self::parse_xml(xml) {
            Ok(r) => r,
            Err(e) => {
                diags.push(XSDDiagnostic::error("parse_error", e));
                return diags;
            }
        };
        self.validate_against_root(schema, &root, &mut diags);
        diags
    }

    /// Validate a pre-parsed root element against `schema`
    /// (C++ `validate(XSDSchema*, const XMLNode&)`).
    pub fn validate_node(&self, schema: &XSDSchema, root: &XmlNode) -> Vec<XSDDiagnostic> {
        let mut diags = Vec::new();
        self.validate_against_root(schema, root, &mut diags);
        diags
    }

    fn validate_against_root(
        &self,
        schema: &XSDSchema,
        root: &XmlNode,
        diags: &mut Vec<XSDDiagnostic>,
    ) {
        match find_global_element(schema, &root.local) {
            None => {
                let mut d = XSDDiagnostic::error(
                    "root_not_found",
                    format!("root element '{}' not declared in schema", root.name),
                );
                d.set_element_qname(root.name.clone());
                diags.push(d);
            }
            Some(elem) => self.validate_element(schema, elem, root, diags),
        }
    }

    fn validate_element<'a>(
        &self,
        schema: &'a XSDSchema,
        elem: &'a XSDElementDeclaration,
        node: &XmlNode,
        diags: &mut Vec<XSDDiagnostic>,
    ) {
        let Some(type_ref) = element_type(schema, elem) else {
            // No resolvable type (e.g. a bare built-in like `xs:anyType`): no constraint.
            return;
        };
        match type_ref {
            XsdTypeRef::Simple(simple) => {
                if !node.children.is_empty() {
                    let mut d = XSDDiagnostic::error(
                        "element_in_simple_type",
                        format!(
                            "element '{}' of simple type must not have children",
                            node.name
                        ),
                    );
                    d.set_element_qname(node.name.clone());
                    diags.push(d);
                    return;
                }
                if self.options.validate_facets {
                    validate_simple_type(simple, &node.text, diags);
                }
            }
            XsdTypeRef::Complex(complex) => {
                if self.options.validate_content_model {
                    self.validate_complex_type(schema, complex, node, diags);
                }
                if self.options.validate_required_attributes {
                    validate_required_attributes(complex, node, diags);
                }
            }
            // An unresolved named type (no local definition): no constraint.
            XsdTypeRef::Named(_) => {}
        }
    }

    fn validate_complex_type<'a>(
        &self,
        schema: &'a XSDSchema,
        type_: &'a XSDComplexTypeDefinition,
        node: &XmlNode,
        diags: &mut Vec<XSDDiagnostic>,
    ) {
        let Some(group) = &type_.compositor else {
            // No content model: empty content unless mixed.
            if !type_.is_mixed && !node.children.is_empty() {
                let mut d = XSDDiagnostic::error(
                    "extra_children",
                    format!(
                        "element '{}' has no content model but has children",
                        node.name
                    ),
                );
                d.set_element_qname(node.name.clone());
                diags.push(d);
            }
            return;
        };

        let particles: Vec<&XsdParticle> = group.particles.iter().collect();
        let kids: Vec<&XmlNode> = node.children.iter().collect();

        match group.kind {
            XsdCompositorKind::Sequence => {
                if !sequence_matches(&particles, &kids) {
                    let mut d = XSDDiagnostic::error(
                        "unexpected_child",
                        format!(
                            "children of element '{}' do not match the sequence content model",
                            node.name
                        ),
                    );
                    d.set_element_qname(node.name.clone());
                    diags.push(d);
                }
            }
            XsdCompositorKind::Choice => {
                for child in &node.children {
                    if !particles.iter().any(|p| particle_accepts(p, child)) {
                        let mut d = XSDDiagnostic::error(
                            "choice_mismatch",
                            format!(
                                "element '{}' not allowed in choice of '{}'",
                                child.name, node.name
                            ),
                        );
                        d.set_element_qname(node.name.clone());
                        diags.push(d);
                    }
                }
            }
            XsdCompositorKind::All => {
                for child in &node.children {
                    if !particles.iter().any(|p| particle_accepts(p, child)) {
                        let mut d = XSDDiagnostic::error(
                            "all_mismatch",
                            format!(
                                "element '{}' not in content model of '{}'",
                                child.name, node.name
                            ),
                        );
                        d.set_element_qname(node.name.clone());
                        diags.push(d);
                    }
                }
            }
        }

        // Recurse into children that resolve to a declared element in the model.
        for child in &node.children {
            if let Some(child_elem) = find_particle_element(&particles, &child.local) {
                self.validate_element(schema, child_elem, child, diags);
            }
        }
    }
}

// ==== simple-type facets (C++ `validateSimpleType`) ====

fn validate_simple_type(
    type_: &XSDSimpleTypeDefinition,
    value: &str,
    diags: &mut Vec<XSDDiagnostic>,
) {
    let len = value.chars().count() as i64;

    // Enumeration is validated once over the union of all enumeration facets.
    let enumerations: Vec<&str> = type_
        .facets
        .iter()
        .filter_map(|f| match f {
            XsdFacet::Enumeration(v) => Some(v.as_str()),
            _ => None,
        })
        .collect();
    if !enumerations.is_empty() && !enumerations.contains(&value) {
        diags.push(XSDDiagnostic::error(
            "enumeration",
            format!("value '{value}' is not in enumeration"),
        ));
    }

    for facet in &type_.facets {
        match facet {
            XsdFacet::MinLength(n) if *n >= 0 && len < *n => {
                diags.push(XSDDiagnostic::error(
                    "minLength",
                    format!("value length {len} < minLength {n}"),
                ));
            }
            XsdFacet::MaxLength(n) if *n >= 0 && len > *n => {
                diags.push(XSDDiagnostic::error(
                    "maxLength",
                    format!("value length {len} > maxLength {n}"),
                ));
            }
            XsdFacet::Length(n) if *n >= 0 && len != *n => {
                diags.push(XSDDiagnostic::error(
                    "length",
                    format!("value length {len} != length {n}"),
                ));
            }
            XsdFacet::Pattern(p) => {
                if !regex_full_match(p, value) {
                    diags.push(XSDDiagnostic::error(
                        "pattern",
                        format!("value '{value}' does not match pattern"),
                    ));
                }
            }
            XsdFacet::MinInclusive(bound) => {
                if let Some(d) = numeric_diag(value, bound, "minInclusive", |v, b| v < b) {
                    diags.push(d);
                }
            }
            XsdFacet::MaxInclusive(bound) => {
                if let Some(d) = numeric_diag(value, bound, "maxInclusive", |v, b| v > b) {
                    diags.push(d);
                }
            }
            XsdFacet::MinExclusive(bound) => {
                if let Some(d) = numeric_diag(value, bound, "minExclusive", |v, b| v <= b) {
                    diags.push(d);
                }
            }
            XsdFacet::MaxExclusive(bound) => {
                if let Some(d) = numeric_diag(value, bound, "maxExclusive", |v, b| v >= b) {
                    diags.push(d);
                }
            }
            // `enumeration` is handled above; `whiteSpace` needs no diagnostic here.
            _ => {}
        }
    }
}

/// Build a numeric-bound diagnostic when `value` and `bound` both parse as
/// integers and `violates(vi, bound)` holds; otherwise `None` (C++ skips
/// non-numeric values).
fn numeric_diag(
    value: &str,
    bound: &str,
    code: &str,
    violates: impl Fn(i64, i64) -> bool,
) -> Option<XSDDiagnostic> {
    let vi: i64 = value.trim().parse().ok()?;
    let bi: i64 = bound.trim().parse().ok()?;
    if violates(vi, bi) {
        Some(XSDDiagnostic::error(
            code,
            format!("value {vi} violates {code} {bi}"),
        ))
    } else {
        None
    }
}

// ==== required attributes (C++ `validateRequiredAttributes`) ====

fn validate_required_attributes(
    type_: &XSDComplexTypeDefinition,
    node: &XmlNode,
    diags: &mut Vec<XSDDiagnostic>,
) {
    for attr in &type_.attributes {
        if attr.use_kind != XsdUse::Required {
            continue;
        }
        let present = node
            .attrs
            .iter()
            .any(|(rawname, _)| local_name(rawname) == attr.name);
        if !present {
            let mut d = XSDDiagnostic::error(
                "required_attr",
                format!(
                    "required attribute '{}' missing on '{}'",
                    attr.name, node.name
                ),
            );
            d.set_element_qname(node.name.clone());
            d.set_attribute_qname(attr.name.clone());
            diags.push(d);
        }
    }
}

// ==== helpers ====

/// The local part of a possibly-prefixed qualified name.
fn local_name(qname: &str) -> &str {
    match qname.rfind(':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

/// Find a global element declaration by local name (C++ `findGlobalElement`).
fn find_global_element<'a>(
    schema: &'a XSDSchema,
    local: &str,
) -> Option<&'a XSDElementDeclaration> {
    schema.elements.iter().find(|e| e.name == local)
}

/// Resolve the type of an element declaration: an inline type wins, otherwise
/// the `type` name is looked up against the schema's global types.
fn element_type<'a>(
    schema: &'a XSDSchema,
    elem: &'a XSDElementDeclaration,
) -> Option<&'a XsdTypeRef> {
    if let Some(inline) = &elem.type_definition {
        return Some(inline);
    }
    let type_name = elem.type_name.as_deref()?;
    schema.type_by_name(local_name(type_name))
}

/// Whether a particle consumes `child` (a nested group / wildcard accepts any).
fn particle_accepts(particle: &XsdParticle, child: &XmlNode) -> bool {
    match &particle.kind {
        XsdParticleKind::Element(e) => e.name == child.local,
        XsdParticleKind::Group(_) => true,
    }
}

/// First element-declaration particle matching `local`.
fn find_particle_element<'a>(
    particles: &[&'a XsdParticle],
    local: &str,
) -> Option<&'a XSDElementDeclaration> {
    particles.iter().find_map(|p| match &p.kind {
        XsdParticleKind::Element(e) if e.name == local => Some(e),
        _ => None,
    })
}

/// Whether `children` conform to an ordered, occurrence-aware `sequence` of
/// particles (greedy with backtracking).
fn sequence_matches(particles: &[&XsdParticle], children: &[&XmlNode]) -> bool {
    fn rec(ps: &[&XsdParticle], cs: &[&XmlNode], pi: usize, ci: usize) -> bool {
        if pi == ps.len() {
            return ci == cs.len();
        }
        let p = ps[pi];
        let min = p.min_occurs.max(0) as usize;
        let max = if p.max_occurs < 0 {
            cs.len()
        } else {
            p.max_occurs.max(0) as usize
        };
        let mut count = 0usize;
        while count < max && ci + count < cs.len() && particle_accepts(p, cs[ci + count]) {
            count += 1;
        }
        if count < min {
            return false;
        }
        // Try to consume as many repetitions as possible, backing off to `min`.
        for c in (min..=count).rev() {
            if rec(ps, cs, pi + 1, ci + c) {
                return true;
            }
        }
        false
    }
    rec(particles, children, 0, 0)
}
