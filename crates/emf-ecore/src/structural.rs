//! Structural features and operations: `EStructuralFeature`, `EAttribute`,
//! `EReference`, `EOperation`, `EParameter`.
//!
//! Port of C++ `emf-ecore` interfaces. A structural feature is the metadata
//! that describes one attribute or reference on an `EClass`, including its
//! bounds, flags and its integer `FeatureID`.

use crate::Val;

/// Kinds of structural feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    /// An attribute (single or multi scalar value).
    Attribute,
    /// A reference to other objects (single or many).
    Reference,
}

/// A structural feature: attribute or reference (C++ `EStructuralFeature`).
#[derive(Debug, Clone)]
pub struct EStructuralFeature {
    /// Feature name, e.g. `"shortName"`.
    name: String,
    /// Attribute vs reference.
    kind: FeatureKind,
    /// Whether changeable.
    changeable: bool,
    /// Whether volatile.
    is_volatile: bool,
    /// Whether transient.
    transient: bool,
    /// Whether unsettable.
    unsettable: bool,
    /// Whether derived.
    derived: bool,
    /// Lower bound (default 0).
    lower_bound: i32,
    /// Upper bound; `-1` (MANY) means unbounded.
    upper_bound: i32,
    /// The string default value literal, if any.
    default_value_literal: Option<String>,
    /// The default value (materialized), if known.
    default_value: Option<Val>,
    /// The integer feature id assigned by the declaring package.
    feature_id: i32,
    /// The meta type (data type name for attributes, target class name for refs).
    type_name: Option<String>,
    /// Whether `ordered`.
    ordered: bool,
    /// Whether `unique`.
    unique: bool,
    /// Whether a containment reference (EMF `EReference.containment`).
    /// Only meaningful when `kind == Reference`.
    containment: bool,
    /// Whether this attribute is the ID (EMF `EAttribute.iD`).
    id: bool,
    /// Whether proxies are resolved for this reference (EMF
    /// `EReference.resolveProxies`, default `true`). Only meaningful for refs.
    resolve_proxies: bool,
}

impl EStructuralFeature {
    /// A fully-configurable constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        kind: FeatureKind,
        lower_bound: i32,
        upper_bound: i32,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            changeable: true,
            is_volatile: false,
            transient: false,
            unsettable: false,
            derived: false,
            lower_bound,
            upper_bound,
            default_value_literal: None,
            default_value: None,
            feature_id: -1,
            type_name: None,
            ordered: true,
            unique: true,
            containment: false,
            id: false,
            resolve_proxies: true,
        }
    }

    /// Attribute shorthand.
    pub fn attribute(name: impl Into<String>) -> Self {
        Self::new(name, FeatureKind::Attribute, 0, 1)
    }

    /// Reference shorthand (single-valued).
    pub fn reference(name: impl Into<String>) -> Self {
        Self::new(name, FeatureKind::Reference, 0, 1)
    }

    /// Multi-valued reference shorthand (`upperBound = MANY`).
    pub fn reference_many(name: impl Into<String>) -> Self {
        Self::new(name, FeatureKind::Reference, 0, -1)
    }

    // ---- accessors ----
    /// Feature name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Whether an attribute or reference.
    pub fn is_reference(&self) -> bool {
        self.kind == FeatureKind::Reference
    }
    /// Whether a containment reference (portal to `EReference.containment`).
    pub fn is_containment(&self) -> bool {
        self.kind == FeatureKind::Reference && self.containment
    }
    /// Set the containment flag (only meaningful for references).
    pub fn set_containment(&mut self, containment: bool) {
        self.containment = containment;
    }
    /// Whether this attribute is an ID (`EAttribute.iD`).
    pub fn is_id(&self) -> bool {
        self.id
    }
    /// Set the ID flag (`EAttribute.iD`).
    pub fn set_id(&mut self, v: bool) {
        self.id = v;
    }
    /// Whether proxies are resolved for this reference (`EReference.resolveProxies`).
    pub fn is_resolve_proxies(&self) -> bool {
        self.resolve_proxies
    }
    /// Set the resolve-proxies flag (`EReference.resolveProxies`).
    pub fn set_resolve_proxies(&mut self, v: bool) {
        self.resolve_proxies = v;
    }
    /// The kind.
    pub fn kind(&self) -> FeatureKind {
        self.kind
    }
    /// Whether changeable.
    pub fn is_changeable(&self) -> bool {
        self.changeable
    }
    /// Set changeable.
    pub fn set_changeable(&mut self, v: bool) {
        self.changeable = v;
    }
    /// Whether volatile.
    pub fn is_volatile(&self) -> bool {
        self.is_volatile
    }
    /// Set volatile.
    pub fn set_volatile(&mut self, v: bool) {
        self.is_volatile = v;
    }
    /// Whether transient.
    pub fn is_transient(&self) -> bool {
        self.transient
    }
    /// Set transient.
    pub fn set_transient(&mut self, v: bool) {
        self.transient = v;
    }
    /// Whether unsettable.
    pub fn is_unsettable(&self) -> bool {
        self.unsettable
    }
    /// Set unsettable.
    pub fn set_unsettable(&mut self, v: bool) {
        self.unsettable = v;
    }
    /// Whether derived.
    pub fn is_derived(&self) -> bool {
        self.derived
    }
    /// Set derived.
    pub fn set_derived(&mut self, v: bool) {
        self.derived = v;
    }
    /// `lowerBound`.
    pub fn lower_bound(&self) -> i32 {
        self.lower_bound
    }
    /// Set `lowerBound`.
    pub fn set_lower_bound(&mut self, v: i32) {
        self.lower_bound = v;
    }
    /// `upperBound`.
    pub fn upper_bound(&self) -> i32 {
        self.upper_bound
    }
    /// Set `upperBound`.
    pub fn set_upper_bound(&mut self, v: i32) {
        self.upper_bound = v;
    }
    /// Whether this is a many-valued feature (`upperBound` != 1).
    pub fn is_many(&self) -> bool {
        self.upper_bound != 1
    }
    /// Whether required (`lowerBound` > 0).
    pub fn is_required(&self) -> bool {
        self.lower_bound > 0
    }
    /// `defaultValueLiteral`.
    pub fn default_value_literal(&self) -> Option<&str> {
        self.default_value_literal.as_deref()
    }
    /// Set `defaultValueLiteral`.
    pub fn set_default_value_literal(&mut self, lit: impl Into<String>) {
        self.default_value_literal = Some(lit.into());
    }
    /// Materialized default value (if known).
    pub fn default_value(&self) -> Option<&Val> {
        self.default_value.as_ref()
    }
    /// Set the default value.
    pub fn set_default_value(&mut self, v: Val) {
        self.default_value = Some(v);
    }
    /// The `featureID`.
    pub fn feature_id(&self) -> i32 {
        self.feature_id
    }
    /// Set `featureID`.
    pub fn set_feature_id(&mut self, id: i32) {
        self.feature_id = id;
    }
    /// The meta type name (data type name for attribute, target class for ref).
    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }
    /// Set the meta type name.
    pub fn set_type_name(&mut self, t: impl Into<String>) {
        self.type_name = Some(t.into());
    }
    /// Whether ordered.
    pub fn is_ordered(&self) -> bool {
        self.ordered
    }
    /// Whether unique.
    pub fn is_unique(&self) -> bool {
        self.unique
    }
}

impl Default for EStructuralFeature {
    fn default() -> Self {
        Self::new("unnamed", FeatureKind::Attribute, 0, 1)
    }
}

/// A map-entry key/value structural feature pair (EMF `EAttribute` with
/// `isID`, and `EReference` with containment/opposite semantics). Here
/// `EAttribute` and `EReference` are represented as [`EStructuralFeature`]
/// plus this descriptor, since both share all field storage above.
#[derive(Debug, Clone, Default)]
pub struct EAttribute {
    feature: EStructuralFeature,
    /// Whether this is the ID attribute.
    is_id: bool,
}

impl EAttribute {
    /// New attribute wrapping the given feature descriptor.
    pub fn new(feature: EStructuralFeature) -> Self {
        Self {
            feature,
            is_id: false,
        }
    }
    /// The underlying structural feature.
    pub fn feature(&self) -> &EStructuralFeature {
        &self.feature
    }
    /// The underlying structural feature, mutably.
    pub fn feature_mut(&mut self) -> &mut EStructuralFeature {
        &mut self.feature
    }
    /// Whether an ID attribute.
    pub fn is_id(&self) -> bool {
        self.is_id || self.feature.is_id()
    }
    /// Set ID (records on the descriptor and on the underlying feature).
    pub fn set_id(&mut self, v: bool) {
        self.is_id = v;
        self.feature.id = v;
    }
}

/// A reference descriptor (EMF `EReference`), layered over `EStructuralFeature`.
#[derive(Debug, Clone, Default)]
pub struct EReference {
    feature: EStructuralFeature,
    /// Whether a containment reference.
    containment: bool,
    /// Whether `resolveProxies`.
    resolve_proxies: bool,
    /// Target class name.
    reference_type: Option<String>,
    /// Opposite feature name (EMF `EReference.eOpposite`).
    opposite: Option<String>,
}

impl EReference {
    /// New reference wrapping the given feature descriptor.
    pub fn new(feature: EStructuralFeature) -> Self {
        Self {
            feature,
            containment: false,
            resolve_proxies: true,
            reference_type: None,
            opposite: None,
        }
    }
    /// The underlying structural feature.
    pub fn feature(&self) -> &EStructuralFeature {
        &self.feature
    }
    /// The underlying structural feature, mutably.
    pub fn feature_mut(&mut self) -> &mut EStructuralFeature {
        &mut self.feature
    }
    /// The opposite feature name (EMF `EReference.eOpposite`), if any.
    pub fn opposite(&self) -> Option<&str> {
        self.opposite.as_deref()
    }
    /// Set the opposite feature name.
    pub fn set_opposite(&mut self, name: impl Into<String>) {
        self.opposite = Some(name.into());
    }
    /// Whether a containment.
    pub fn is_containment(&self) -> bool {
        self.containment
    }
    /// Set containment.
    pub fn set_containment(&mut self, v: bool) {
        self.containment = v;
    }
    /// Whether `resolveProxies`.
    pub fn is_resolve_proxies(&self) -> bool {
        self.resolve_proxies
    }
    /// Whether `container` (derived inverse of containment).
    pub fn is_container(&self) -> bool {
        self.containment
    }
    /// The target class name.
    pub fn reference_type(&self) -> Option<&str> {
        self.reference_type.as_deref()
    }
    /// Set the target class name.
    pub fn set_reference_type(&mut self, t: impl Into<String>) {
        self.reference_type = Some(t.into());
    }
}

/// An operation on a class (EMF `EOperation`) — signature metadata.
#[derive(Debug, Clone, Default)]
pub struct EOperation {
    /// Operation name.
    name: String,
    /// Parameter names.
    parameters: Vec<String>,
    /// Whether abstract.
    is_abstract: bool,
    /// The operation id within its class.
    operation_id: i32,
    /// Return type data-type name, if known.
    return_type: Option<String>,
}

impl EOperation {
    /// New operation.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    /// Operation name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Parameter names.
    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }
    /// Append a parameter name.
    pub fn add_parameter(&mut self, name: impl Into<String>) {
        self.parameters.push(name.into());
    }
    /// Whether abstract.
    pub fn is_abstract(&self) -> bool {
        self.is_abstract
    }
    /// Whether an operation ever has a return value.
    pub fn return_type(&self) -> Option<&str> {
        self.return_type.as_deref()
    }
    /// Set the return type name.
    pub fn set_return_type(&mut self, t: impl Into<String>) {
        self.return_type = Some(t.into());
    }
    /// Operation id.
    pub fn operation_id(&self) -> i32 {
        self.operation_id
    }
    /// Set operation id.
    pub fn set_operation_id(&mut self, id: i32) {
        self.operation_id = id;
    }
}

/// A parameter of an operation (EMF `EParameter`).
#[derive(Debug, Clone, Default)]
pub struct EParameter {
    /// Parameter name.
    name: String,
    /// Type name (data type or class).
    type_name: Option<String>,
}

impl EParameter {
    /// New parameter.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: None,
        }
    }
    /// Parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Type name.
    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }
}
