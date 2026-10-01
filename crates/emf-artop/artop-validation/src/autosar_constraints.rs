//! AUTOSAR business constraints living *above* the generic `emf-validation`
//! base, aligned to the real artop layering `org.artop.aal.*.constraints`
//! (AUTOSAR-domain constraint bundles layering over the EMF validation
//! runtime instead of living in it).
//!
//! All constraints read features reflectively through [`emf_common::eobject::EObject`]
//! (`shortName` / `uuid` / `category` / cross references), so they apply both to
//! real AUTOSAR models and to the dynamic test model.
//!
//! # Constraints
//! - `autosar.short_name_non_empty[.live]` — Referrable.shortName non-empty (BATCH+LIVE)
//! - `autosar.short_name_unique_in_parent` — same-class sibling shortName unique (BATCH)
//! - `autosar.uuid_non_empty[.live]` — Identifiable.uuid non-empty (BATCH+LIVE)
//! - `autosar.category_required` — required `category` non-empty (BATCH)
//! - `autosar.no_unresolved_proxy[.live]` — no dangling proxy in non-containment refs (BATCH+LIVE)
//!
//! UUID global uniqueness is a whole-model sweep ([`validate_uuid_uniqueness`]),
//! not a per-object constraint — matching C++ (`validateUuidUniqueness`).
//!
//! Plus `register_ecuc_constraints` (all 49 ECUC constraints, aligned
//! one-for-one with the C++ `registerEcucConstraints`) and the standalone
//! [`validate_uuid_uniqueness`] tree sweep.

use emf_common::diagnostic::{Diagnostic, Severity as CommonSeverity};
use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use emf_validation::constraint::{Constraint, ConstraintMode, Severity};
use emf_validation::diagnostician::map_severity;
use emf_validation::e_validator::EValidator;
use std::collections::HashMap;

// ===================== reflective helpers =====================

/// Read a string feature; `None` when the feature is absent or not a string.
///
/// A feature the metamodel *declares* but the object leaves unset reads as the
/// empty string, matching the C++ generated model (whose EAttributes default to
/// `""`, so C++ `readStringAttr` yields `""` rather than "absent"). Without
/// this, "non-empty" constraints would silently pass on real arxml where the
/// attribute is simply omitted.
fn read_str(obj: &dyn EObject, name: &str) -> Option<String> {
    match obj.e_get(name).and_then(|v| v.as_str().map(String::from)) {
        Some(s) => Some(s),
        None if obj.e_has_feature(name) => Some(String::new()),
        None => None,
    }
}

/// Read a numeric feature (`Int` / `Double`); `None` when absent or non-numeric.
fn read_numeric(obj: &dyn EObject, name: &str) -> Option<f64> {
    obj.e_get(name)
        .and_then(|v| v.as_int().map(|i| i as f64).or_else(|| v.as_double()))
}

/// Read a single-valued object reference; `None` when absent / null / not a ref.
fn read_object_ref(obj: &dyn EObject, name: &str) -> Option<ObjectRef> {
    obj.e_get(name).and_then(|v| v.as_object().cloned())
}

/// Number of elements in a multi-valued reference feature.
fn ref_list_size(obj: &dyn EObject, name: &str) -> usize {
    obj.e_get(name)
        .and_then(|v| v.as_list().map(|l| l.len()))
        .unwrap_or(0)
}

/// Trait-object identity; `true` when `a` and `b` are the same underlying object.
fn same_object(a: &dyn EObject, b: &dyn EObject) -> bool {
    std::ptr::eq(
        a as *const dyn EObject as *const (),
        b as *const dyn EObject as *const (),
    )
}

// ===================== per-object AUTOSAR evaluators =====================

/// `shortName` must not be empty (no `shortName` feature ⇒ not applicable).
fn short_name_non_empty_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "shortName") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

/// `shortName` must be unique among siblings of the *same* class under a parent.
fn short_name_unique_in_parent_eval(obj: &dyn EObject) -> bool {
    let sn = match read_str(obj, "shortName") {
        Some(s) if !s.is_empty() => s,
        _ => return true,
    };
    let Some(parent) = obj.e_container() else {
        return true;
    };
    let my_type = obj.e_class().to_string();
    let parent_ref = parent.borrow();
    for sibling in parent_ref.e_contents() {
        let sb = sibling.borrow();
        if same_object(&*sb, obj) {
            continue;
        }
        if sb.e_class() != my_type {
            continue; // only same-class siblings compete
        }
        if let Some(sn2) = read_str(&*sb, "shortName") {
            if sn2 == sn {
                return false;
            }
        }
    }
    true
}

/// `uuid` must not be empty (no `uuid` feature ⇒ not applicable).
fn uuid_non_empty_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "uuid") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

/// Required `category` must be non-empty.
///
/// Only applies when the metamodel declares `category` with `lowerBound >= 1`
/// (C++ `categoryLowerBound(obj) < 1 ⇒ not applicable`); a declared-but-unset
/// category then reads as `""` and is a violation.
fn category_required_eval(obj: &dyn EObject) -> bool {
    if obj.e_feature_lower_bound("category") < 1 {
        return true;
    }
    match read_str(obj, "category") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

/// No non-containment reference target may be an unresolved proxy.
///
/// `e_cross_references()` is the `EObject`-trait reflection surface that
/// enumerates non-containment reference targets (single object or list); a
/// proxy target (`e_is_proxy()` true) is a dangling cross-resource reference.
fn no_unresolved_proxy_eval(obj: &dyn EObject) -> bool {
    for xref in obj.e_cross_references() {
        if xref.borrow().e_is_proxy() {
            return false;
        }
    }
    true
}

// ===================== UUID global-uniqueness (model-level) =====================

/// Single DFS over `root`'s containment tree reporting every empty uuid and
/// every duplicate uuid (the first occurrence of a duplicated value is left
/// silent; subsequent occurrences are reported). `source = "AutosarUuidGloballyUnique"`.
pub fn validate_uuid_uniqueness(root: &dyn EObject) -> Vec<Diagnostic> {
    fn visit(
        obj: &dyn EObject,
        first_owners: &mut HashMap<String, usize>,
        result: &mut Vec<Diagnostic>,
        count: usize,
    ) -> usize {
        let mut next = count;
        if let Some(u) = read_str(obj, "uuid") {
            if u.is_empty() {
                result.push(Diagnostic::new(
                    CommonSeverity::Error,
                    "AutosarUuidGloballyUnique",
                    0,
                    "AUTOSAR Identifiable.uuid must not be empty",
                ));
            } else if let Some(&first) = first_owners.get(&u) {
                if first != next {
                    result.push(Diagnostic::new(
                        CommonSeverity::Error,
                        "AutosarUuidGloballyUnique",
                        0,
                        format!(
                            "AUTOSAR Identifiable.uuid must be globally unique: duplicate uuid '{u}'"
                        ),
                    ));
                }
            } else {
                first_owners.insert(u, next);
            }
        }
        for child in obj.e_contents() {
            next += 1;
            next = visit(&*child.borrow(), first_owners, result, next);
        }
        next
    }
    let mut first_owners = HashMap::new();
    let mut result = Vec::new();
    visit(root, &mut first_owners, &mut result, 0);
    result
}

// ===================== named-source validation helpers =====================
//
// The generic `emf-validation` `EValidator::validate_mode` stamps every
// diagnostic with the process-wide `DIAGNOSTIC_SOURCE` constant. The business
// layer instead wants each diagnostic's `source` to be the *constraint name*
// (e.g. `AutosarShortNameNonEmpty`), matching C++ `AutosarConstraints.cpp` and
// real artop plugin behaviour. These helpers re-run a registered validator and
// re-source each diagnostic with `Constraint::name()`, without modifying the
// `emf-validation` public API.

/// Validate a single object against `validator` (Optionally filtered by mode),
/// emitting diagnostics whose `source` is the violating constraint's name.
pub fn validate_named_single(
    validator: &EValidator,
    mode: Option<ConstraintMode>,
    target: &dyn EObject,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for c in validator.constraints() {
        if mode.is_some_and(|m| c.mode() != m) {
            continue;
        }
        if !c.evaluate(target) {
            out.push(Diagnostic::new(
                map_severity(c.severity()),
                c.name(),
                0,
                c.message(),
            ));
        }
    }
    out
}

/// Validate `root` and its whole containment tree, named-source diagnostics.
pub fn validate_named_tree(
    validator: &EValidator,
    mode: Option<ConstraintMode>,
    root: &dyn EObject,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    collect_named_tree(validator, mode, root, &mut out);
    out
}

fn collect_named_tree(
    validator: &EValidator,
    mode: Option<ConstraintMode>,
    obj: &dyn EObject,
    out: &mut Vec<Diagnostic>,
) {
    out.extend(validate_named_single(validator, mode, obj));
    for child in obj.e_contents() {
        collect_named_tree(validator, mode, &*child.borrow(), out);
    }
}

// ===================== registration =====================

/// Register the core AUTOSAR business constraints (idempotent by id).
///
/// `shortName`/`uuid` non-empty and `no_unresolved_proxy` are registered as
/// BATCH+LIVE dual mode; the uniqueness and `category` constraints are BATCH.
pub fn register_autosar_constraints(validator: &mut EValidator) {
    // shortName non-empty: BATCH + LIVE
    validator.add_constraint(
        Box::new(short_name_non_empty_eval),
        "autosar.short_name_non_empty",
        "AutosarShortNameNonEmpty",
        "AUTOSAR Referrable.shortName must not be empty",
        Severity::Error,
        ConstraintMode::Batch,
    );
    validator.add_constraint(
        Box::new(short_name_non_empty_eval),
        "autosar.short_name_non_empty.live",
        "AutosarShortNameNonEmpty",
        "AUTOSAR Referrable.shortName must not be empty",
        Severity::Error,
        ConstraintMode::Live,
    );

    // shortName same-class sibling uniqueness: BATCH
    validator.add_constraint(
        Box::new(short_name_unique_in_parent_eval),
        "autosar.short_name_unique_in_parent",
        "AutosarShortNameUniqueInParent",
        "AUTOSAR shortName must be unique among siblings of the same type within a parent",
        Severity::Error,
        ConstraintMode::Batch,
    );

    // uuid non-empty: BATCH + LIVE
    validator.add_constraint(
        Box::new(uuid_non_empty_eval),
        "autosar.uuid_non_empty",
        "AutosarUuidNonEmpty",
        "AUTOSAR Identifiable.uuid must not be empty",
        Severity::Error,
        ConstraintMode::Batch,
    );
    validator.add_constraint(
        Box::new(uuid_non_empty_eval),
        "autosar.uuid_non_empty.live",
        "AutosarUuidNonEmpty",
        "AUTOSAR Identifiable.uuid must not be empty",
        Severity::Error,
        ConstraintMode::Live,
    );

    // category required non-empty: BATCH
    validator.add_constraint(
        Box::new(category_required_eval),
        "autosar.category_required",
        "AutosarCategoryRequired",
        "AUTOSAR category (lowerBound>=1) must not be empty",
        Severity::Error,
        ConstraintMode::Batch,
    );

    // no unresolved proxy: BATCH + LIVE
    validator.add_constraint(
        Box::new(no_unresolved_proxy_eval),
        "autosar.no_unresolved_proxy",
        "AutosarNoUnresolvedProxy",
        "AUTOSAR cross-resource references must be resolved (no dangling proxy)",
        Severity::Warning,
        ConstraintMode::Batch,
    );
    validator.add_constraint(
        Box::new(no_unresolved_proxy_eval),
        "autosar.no_unresolved_proxy.live",
        "AutosarNoUnresolvedProxy",
        "AUTOSAR cross-resource references must be resolved (no dangling proxy)",
        Severity::Warning,
        ConstraintMode::Live,
    );
    // NOTE: unlike the other constraints, uuid global uniqueness is *not*
    // registered per-object here. It is a whole-model sweep run once via
    // [`validate_uuid_uniqueness`], matching C++ `registerAutosarConstraints`
    // (which likewise leaves it to the standalone `validateUuidUniqueness`).
    // Registering it per object would re-scan every subtree and double-report.
}

// ===================== ECUC constraints (all 49, aligned with C++) =====================

/// Copy of the C++ `registerEcucConstraint` scoping helper: register a
/// clientContext-style class-substring filter so non-matching objects are
/// skipped before their evaluator runs.
fn register_ecuc(
    validator: &mut EValidator,
    eval: Box<dyn Fn(&dyn EObject) -> bool>,
    id: &'static str,
    name: &'static str,
    message: &'static str,
    severity: Severity,
    target: &str,
) {
    let mut c = Constraint::new(eval, id, name, message, severity, ConstraintMode::Batch);
    c.add_target_class_name(target);
    validator.register_constraint(c);
}

// ---- evaluators (aligned one-for-one with C++ AutosarConstraints.cpp) ----

/// Read a boolean feature; `None` when the feature is absent or non-boolean.
fn read_bool(obj: &dyn EObject, name: &str) -> Option<bool> {
    obj.e_get(name).and_then(|v| v.as_bool())
}

/// Count `name` elements and check the metamodel lower bound (C++
/// `constraint: count >= sf->getLowerBound()`; `0` when metadata is absent).
fn satisfies_lower_multiplicity(obj: &dyn EObject, name: &str) -> bool {
    ref_list_size(obj, name) as i32 >= obj.e_feature_lower_bound(name)
}

/// Count `name` elements and check the metamodel upper bound (C++
/// `count <= sf->getUpperBound()`; `-1` = unbounded passes).
fn satisfies_upper_multiplicity(obj: &dyn EObject, name: &str) -> bool {
    let upper = obj.e_feature_upper_bound(name);
    upper < 0 || ref_list_size(obj, name) as i32 <= upper
}

// ===== #1 EcucModuleConfigurationValuesBasicConstraint =====
fn ecuc_module_config_has_definition_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("definition") {
        return true;
    }
    read_object_ref(obj, "definition").is_some()
}

// ===== #2 GContainerBasicConstraint =====
fn ecuc_container_value_has_definition_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("definition") {
        return true;
    }
    read_object_ref(obj, "definition").is_some()
}

// ===== #3 GParamConfMultiplicityBasicConstraint =====
fn ecuc_definition_element_multiplicity_basic_eval(obj: &dyn EObject) -> bool {
    match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l >= 0.0,
        None => true,
    }
}

// ===== #4 EcucNumericalParamValueBasicConstraint =====
fn ecuc_numerical_param_value_within_limits_eval(obj: &dyn EObject) -> bool {
    let val = match read_numeric(obj, "value") {
        Some(v) => v,
        None => return true,
    };
    let def = match read_object_ref(obj, "definition") {
        Some(d) => d,
        None => return true,
    };
    let def = def.borrow();
    if let Some(lower) = read_numeric(&*def, "lowerLimit") {
        if val < lower {
            return false;
        }
    }
    if let Some(upper) = read_numeric(&*def, "upperLimit") {
        if val > upper {
            return false;
        }
    }
    true
}

// ===== #5 EcucTextualParamValueBasicConstraint =====
fn ecuc_textual_param_value_non_empty_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "value") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

// ===== #6 GReferenceValueBasicConstraint =====
fn ecuc_reference_value_has_definition_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("definition") {
        return true;
    }
    read_object_ref(obj, "definition").is_some()
}

// ===== #7 EcucInstanceReferenceValueBasicConstraint =====
fn ecuc_instance_reference_value_complete_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("definition") {
        return true;
    }
    if read_object_ref(obj, "definition").is_none() {
        return false;
    }
    if !obj.e_has_feature("value") {
        return true;
    }
    // C++ fails only when the `value` feature's runtime value is a null
    // `EObject*`; an unset single reference reads back as `Val::Null`.
    !matches!(obj.e_get("value"), Some(Val::Null))
}

// ===== #8 GReferenceDefBasicConstraint =====
fn ecuc_reference_def_has_destination_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("destination") {
        return true;
    }
    read_object_ref(obj, "destination").is_some()
}

// ===== #9 GChoiceReferenceDefBasicConstraint =====
fn ecuc_choice_reference_def_has_destination_eval(obj: &dyn EObject) -> bool {
    if obj.e_has_feature("destinations") {
        return ref_list_size(obj, "destinations") > 0;
    }
    if obj.e_has_feature("destination") {
        return read_object_ref(obj, "destination").is_some();
    }
    true
}

// ===== #10 GEnumerationParamDefEnumerationLiteralConstraint =====
fn ecuc_enumeration_param_def_has_literals_eval(obj: &dyn EObject) -> bool {
    ref_list_size(obj, "literals") > 0
}

// ===== #11 GConfigParameterSymbolicNameValueConstraint =====
fn ecuc_parameter_def_symbolic_name_non_empty_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "symbolicName") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

// ===== #12 EcucAbstractReferenceValueBasicConstraint =====
fn ecuc_abstract_reference_value_has_definition_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("definition") {
        return true;
    }
    read_object_ref(obj, "definition").is_some()
}

// ===== #13 EcucParameterValueBasicConstraint =====
fn ecuc_parameter_value_basic_eval(obj: &dyn EObject) -> bool {
    if obj.e_has_feature("definition") && read_object_ref(obj, "definition").is_none() {
        return false;
    }
    if obj.e_has_feature("value") {
        if let Some(v) = obj.e_get("value") {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return true;
                }
            } else if v.as_int().is_some() || v.as_double().is_some() {
                return true;
            }
        }
    }
    ref_list_size(obj, "references") > 0
}

// ===== #14 EcucFloatParamDefLowerLimitConstraint =====
fn ecuc_float_param_def_lower_limit_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("lowerLimit") {
        return true;
    }
    read_numeric(obj, "lowerLimit").is_some()
}

// ===== #15 EcucValueCollectionModuleConfigurationLowerMultiplicityConstraint =====
fn ecuc_value_collection_lower_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("moduleConfigurationValues") {
        return true;
    }
    satisfies_lower_multiplicity(obj, "moduleConfigurationValues")
}

// ===== #16 EcucValueCollectionModuleConfigurationUpperMultiplicityConstraint =====
fn ecuc_value_collection_upper_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("moduleConfigurationValues") {
        return true;
    }
    satisfies_upper_multiplicity(obj, "moduleConfigurationValues")
}

// ===== #17 EcucFloatParamDefUpperLimitConstraint =====
fn ecuc_float_param_def_upper_limit_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("upperLimit") {
        return true;
    }
    read_numeric(obj, "upperLimit").is_some()
}

// ===== #18 ModuleConfigurationSubContainerMultiplicityConstraint =====
fn ecuc_module_config_sub_container_lower_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("subContainers") {
        return true;
    }
    satisfies_lower_multiplicity(obj, "subContainers")
}

// ===== #19 ContainerSubContainerMultiplicityConstraint =====
fn ecuc_container_sub_container_lower_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("subContainers") {
        return true;
    }
    satisfies_lower_multiplicity(obj, "subContainers")
}

// ===== #20 GContainerParameterValueMultiplicityConstraint =====
fn ecuc_container_parameter_value_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("parameterValues") {
        return true;
    }
    satisfies_lower_multiplicity(obj, "parameterValues")
}

// ===== #21 GContainerReferenceValueMultiplicityConstraint =====
fn ecuc_container_reference_value_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("referenceValues") {
        return true;
    }
    satisfies_lower_multiplicity(obj, "referenceValues")
}

// ===== #22 GContainerDefLowerMultiplicityConstraint =====
fn ecuc_container_def_lower_multiplicity_eval(obj: &dyn EObject) -> bool {
    match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l >= 0.0,
        None => true,
    }
}

// ===== #23 GConfigParameterLowerMultiplicityConstraint =====
fn ecuc_parameter_def_lower_multiplicity_eval(obj: &dyn EObject) -> bool {
    match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l >= 0.0,
        None => true,
    }
}

// ===== #24 GConfigParameterUpperMultiplicityConstraint =====
fn ecuc_parameter_def_upper_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("upperMultiplicity") {
        return true;
    }
    match read_numeric(obj, "upperMultiplicity") {
        Some(upper) => upper < 0.0 || upper >= 1.0,
        None => false,
    }
}

// ===== #25 GConfigReferenceLowerMultiplicityConstraint =====
fn ecuc_abstract_reference_def_lower_multiplicity_eval(obj: &dyn EObject) -> bool {
    match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l >= 0.0,
        None => true,
    }
}

// ===== #26 GConfigReferenceUpperMultiplicityConstraint =====
fn ecuc_abstract_reference_def_upper_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("upperMultiplicity") {
        return true;
    }
    match read_numeric(obj, "upperMultiplicity") {
        Some(upper) => upper < 0.0 || upper >= 1.0,
        None => false,
    }
}

// ===== #27 GParamConfContainerDefInChoiceContainerDefMultiplicityConstraint =====
fn ecuc_param_conf_container_def_in_choice_multiplicity_eval(obj: &dyn EObject) -> bool {
    let lower = match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l,
        None => return true,
    };
    let upper = match read_numeric(obj, "upperMultiplicity") {
        Some(u) => u,
        None => return true,
    };
    if upper < 0.0 {
        return true;
    }
    lower <= upper
}

// ===== #28 GParamConfMultiplicityConsistencyConstraint =====
fn ecuc_param_conf_multiplicity_consistency_eval(obj: &dyn EObject) -> bool {
    let lower = match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l,
        None => return true,
    };
    let upper = match read_numeric(obj, "upperMultiplicity") {
        Some(u) => u,
        None => return true,
    };
    if upper < 0.0 {
        return true;
    }
    lower <= upper
}

// ===== #29 GModuleConfigurationChoiceContainerDefMultiplicityConstraint =====
fn ecuc_module_config_choice_container_def_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("choiceContainerDef") {
        return true;
    }
    satisfies_lower_multiplicity(obj, "choiceContainerDef")
}

// ===== #30 GContainerDefUpperMultiplicityConstraint =====
fn ecuc_container_def_upper_multiplicity_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("upperMultiplicity") {
        return true;
    }
    match read_numeric(obj, "upperMultiplicity") {
        Some(upper) => upper < 0.0 || upper >= 1.0,
        None => false,
    }
}

// ===== #31 EcucConfigParameterDefaultValueConstraint =====
fn ecuc_parameter_def_default_value_valid_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("defaultValue") {
        return true;
    }
    read_str(obj, "defaultValue").is_some() || read_numeric(obj, "defaultValue").is_some()
}

// ===== #32 GEnumerationParamDefDefaultValueConstraint =====
fn ecuc_enumeration_param_def_default_value_eval(obj: &dyn EObject) -> bool {
    let dv = match read_str(obj, "defaultValue") {
        Some(s) => s,
        None => return true,
    };
    if dv.is_empty() {
        return true;
    }
    if !obj.e_has_feature("literals") {
        return true;
    }
    let Some(list_val) = obj.e_get("literals") else {
        return true;
    };
    let Some(items) = list_val.as_list() else {
        return true;
    };
    for lit in items {
        let Some(o) = lit.as_object() else {
            continue;
        };
        let b = o.borrow();
        if read_str(&*b, "shortName").as_deref() == Some(dv.as_str()) {
            return true;
        }
        if read_str(&*b, "literal").as_deref() == Some(dv.as_str()) {
            return true;
        }
    }
    false
}

// ===== #33 EcucFloatParamDefDefaultValueConstraint =====
fn ecuc_float_param_def_default_value_eval(obj: &dyn EObject) -> bool {
    let dv = match read_numeric(obj, "defaultValue") {
        Some(v) => v,
        None => return true,
    };
    if let Some(lower) = read_numeric(obj, "lowerLimit") {
        if dv < lower {
            return false;
        }
    }
    if let Some(upper) = read_numeric(obj, "upperLimit") {
        if dv > upper {
            return false;
        }
    }
    true
}

// ===== #34 EcucIntegerParamDefDefaultValueConstraint =====
fn ecuc_integer_param_def_default_value_eval(obj: &dyn EObject) -> bool {
    let dv = match read_numeric(obj, "defaultValue") {
        Some(v) => v,
        None => return true,
    };
    if let Some(lower) = read_numeric(obj, "lowerLimit") {
        if dv < lower {
            return false;
        }
    }
    if let Some(upper) = read_numeric(obj, "upperLimit") {
        if dv > upper {
            return false;
        }
    }
    true
}

// ===== #35 EcucLinkerSymbolDefDefaultValueConstraint =====
fn ecuc_linker_symbol_def_default_value_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("defaultValue") {
        return true;
    }
    match read_str(obj, "defaultValue") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

// ===== #36 EcucStringParamDefDefaultValueConstraint =====
fn ecuc_string_param_def_default_value_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("defaultValue") {
        return true;
    }
    match read_str(obj, "defaultValue") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

// ===== #37 EcucFunctionNameDefDefaultValueConstraint =====
fn ecuc_function_name_def_default_value_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("defaultValue") {
        return true;
    }
    match read_str(obj, "defaultValue") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

// ===== #38 GModuleDefContainerDefinitionMissingConstraint =====
fn ecuc_module_def_container_definition_missing_eval(obj: &dyn EObject) -> bool {
    if obj.e_has_feature("containerDef") {
        return ref_list_size(obj, "containerDef") > 0;
    }
    if obj.e_has_feature("container") {
        return ref_list_size(obj, "container") > 0;
    }
    true
}

// ===== #39 GParamConfContainerDefConfigReferenceMissingConstraint =====
fn ecuc_param_conf_container_def_config_reference_missing_eval(obj: &dyn EObject) -> bool {
    let mut has_feature = false;
    if obj.e_has_feature("configReference") {
        has_feature = true;
        if ref_list_size(obj, "configReference") > 0 {
            return true;
        }
    }
    if obj.e_has_feature("reference") {
        has_feature = true;
        if ref_list_size(obj, "reference") > 0 {
            return true;
        }
    }
    if obj.e_has_feature("references") {
        has_feature = true;
        if ref_list_size(obj, "references") > 0 {
            return true;
        }
    }
    !has_feature
}

// ===== #40 GParamConfContainerDefConfigParameterMissingConstraint =====
fn ecuc_param_conf_container_def_config_parameter_missing_eval(obj: &dyn EObject) -> bool {
    let mut has_feature = false;
    if obj.e_has_feature("configParameter") {
        has_feature = true;
        if ref_list_size(obj, "configParameter") > 0 {
            return true;
        }
    }
    if obj.e_has_feature("parameter") {
        has_feature = true;
        if ref_list_size(obj, "parameter") > 0 {
            return true;
        }
    }
    if obj.e_has_feature("parameters") {
        has_feature = true;
        if ref_list_size(obj, "parameters") > 0 {
            return true;
        }
    }
    !has_feature
}

// ===== #41 GContainerDefContainerDefinitionMissingConstraint =====
fn ecuc_container_def_container_definition_missing_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("containerDefinition") {
        return true;
    }
    read_object_ref(obj, "containerDefinition").is_some()
}

// ===== #42 EcucIntegerParamDefLowerLimitConstraint =====
fn ecuc_integer_param_def_lower_limit_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("lowerLimit") {
        return true;
    }
    read_numeric(obj, "lowerLimit").is_some()
}

// ===== #43 EcucIntegerParamDefUpperLimitConstraint =====
fn ecuc_integer_param_def_upper_limit_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("upperLimit") {
        return true;
    }
    read_numeric(obj, "upperLimit").is_some()
}

// ===== #44 EcucParameterDefSymbolicNameValueModifyConstraint =====
fn ecuc_parameter_def_symbolic_name_modify_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "symbolicName") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

// ===== #45 EcucContainerDefPostBuildChangeableModifyConstraint =====
fn ecuc_container_def_post_build_changeable_modify_eval(obj: &dyn EObject) -> bool {
    // C++ only verifies the `postBuildChangeable` flag is readable; there is no
    // additional model invariant, so applicability is the whole check.
    if !obj.e_has_feature("postBuildChangeable") {
        return true;
    }
    read_bool(obj, "postBuildChangeable").is_some()
}

// ===== #46 EcucImplementationConfigurationClassLinkTimeConstraint =====
fn ecuc_implementation_config_class_link_time_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "implementationConfigClass") {
        Some(cls) => cls != "LinkTime",
        None => true,
    }
}

// ===== #47 EcucImplementationConfigurationClassPreCompileConstraint =====
fn ecuc_implementation_config_class_pre_compile_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "implementationConfigClass") {
        Some(cls) => cls != "PreCompile",
        None => true,
    }
}

// ===== #48 EcucParamConfContainerDefMultipleConfigurationModifyConstraint =====
fn ecuc_param_conf_container_def_multiple_configuration_modify_eval(obj: &dyn EObject) -> bool {
    if !obj.e_has_feature("multipleConfiguration") {
        return true;
    }
    match read_bool(obj, "multipleConfiguration") {
        Some(true) => {}
        _ => return true,
    }
    if let (Some(lower), Some(upper)) = (
        read_numeric(obj, "lowerMultiplicity"),
        read_numeric(obj, "upperMultiplicity"),
    ) {
        if upper >= 0.0 {
            return lower <= upper;
        }
    }
    true
}

// ===== #49 EcucParameterDefImplConfigClassConstraint =====
fn ecuc_parameter_def_impl_config_class_eval(obj: &dyn EObject) -> bool {
    // Feature-existence check only (C++ returns true whenever the feature is
    // present), so this is satisfied for every applicable object.
    obj.e_has_feature("implementationConfigClass") || true
}

/// Register all 49 ECUC constraints, one-for-one with the C++
/// `registerEcucConstraints` (same order, ids, names, messages, severities and
/// `clientContext`-style target class names).
pub fn register_ecuc_constraints(validator: &mut EValidator) {
    // 1. EcucModuleConfigurationValuesBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_module_config_has_definition_eval),
        "autosar.ecuc.module_configuration_values_basic",
        "EcucModuleConfigurationValuesBasicConstraint",
        "EcucModuleConfigurationValues must reference a valid ModuleDef definition",
        Severity::Error,
        "EcucModuleConfigurationValues",
    );
    // 2. GContainerBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_value_has_definition_eval),
        "autosar.ecuc.g_container_basic",
        "GContainerBasicConstraint",
        "EcucContainerValue must reference a valid ContainerDef definition",
        Severity::Error,
        "EcucContainerValue",
    );
    // 3. GParamConfMultiplicityBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_definition_element_multiplicity_basic_eval),
        "autosar.ecuc.g_param_conf_multiplicity_basic",
        "GParamConfMultiplicityBasicConstraint",
        "EcucDefinitionElement lowerMultiplicity must be a non-negative value",
        Severity::Error,
        "EcucDefinitionElement",
    );
    // 4. EcucNumericalParamValueBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_numerical_param_value_within_limits_eval),
        "autosar.ecuc.numerical_param_value_basic",
        "EcucNumericalParamValueBasicConstraint",
        "EcucNumericalParamValue value must be within ParameterDef lower/upper limits",
        Severity::Error,
        "EcucNumericalParamValue",
    );
    // 5. EcucTextualParamValueBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_textual_param_value_non_empty_eval),
        "autosar.ecuc.textual_param_value_basic",
        "EcucTextualParamValueBasicConstraint",
        "EcucTextualParamValue value must not be empty",
        Severity::Error,
        "EcucTextualParamValue",
    );
    // 6. GReferenceValueBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_reference_value_has_definition_eval),
        "autosar.ecuc.g_reference_value_basic",
        "GReferenceValueBasicConstraint",
        "EcucReferenceValue must reference a valid ReferenceDef definition",
        Severity::Error,
        "EcucReferenceValue",
    );
    // 7. EcucInstanceReferenceValueBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_instance_reference_value_complete_eval),
        "autosar.ecuc.instance_reference_value_basic",
        "EcucInstanceReferenceValueBasicConstraint",
        "EcucInstanceReferenceValue must have both definition and value references",
        Severity::Error,
        "EcucInstanceReferenceValue",
    );
    // 8. GReferenceDefBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_reference_def_has_destination_eval),
        "autosar.ecuc.g_reference_def_basic",
        "GReferenceDefBasicConstraint",
        "EcucReferenceDef must reference a valid destination",
        Severity::Error,
        "EcucReferenceDef",
    );
    // 9. GChoiceReferenceDefBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_choice_reference_def_has_destination_eval),
        "autosar.ecuc.g_choice_reference_def_basic",
        "GChoiceReferenceDefBasicConstraint",
        "EcucChoiceReferenceDef must define at least one destination",
        Severity::Error,
        "EcucChoiceReferenceDef",
    );
    // 10. GEnumerationParamDefEnumerationLiteralConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_enumeration_param_def_has_literals_eval),
        "autosar.ecuc.g_enumeration_param_def_enumeration_literal",
        "GEnumerationParamDefEnumerationLiteralConstraint",
        "EcucEnumerationParamDef must define at least one literal",
        Severity::Warning,
        "EcucEnumerationParamDef",
    );
    // 11. GConfigParameterSymbolicNameValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_symbolic_name_non_empty_eval),
        "autosar.ecuc.g_config_parameter_symbolic_name_value",
        "GConfigParameterSymbolicNameValueConstraint",
        "EcucParameterDef symbolicName must not be empty when present",
        Severity::Error,
        "EcucParameterDef",
    );
    // 12. EcucAbstractReferenceValueBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_abstract_reference_value_has_definition_eval),
        "autosar.ecuc.abstract_reference_value_basic",
        "EcucAbstractReferenceValueBasicConstraint",
        "EcucAbstractReferenceValue must reference a valid definition",
        Severity::Warning,
        "EcucAbstractReferenceValue",
    );
    // 13. EcucParameterValueBasicConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_value_basic_eval),
        "autosar.ecuc.parameter_value_basic",
        "EcucParameterValueBasicConstraint",
        "EcucParameterValue must have both a definition and a value",
        Severity::Warning,
        "EcucParameterValue",
    );
    // 14. EcucFloatParamDefLowerLimitConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_float_param_def_lower_limit_eval),
        "autosar.ecuc.float_param_def_lower_limit",
        "EcucFloatParamDefLowerLimitConstraint",
        "EcucFloatParamDef lowerLimit must be a valid numerical value",
        Severity::Error,
        "EcucFloatParamDef",
    );
    // 15. EcucValueCollectionModuleConfigurationLowerMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_value_collection_lower_multiplicity_eval),
        "autosar.ecuc.value_collection_module_configuration_lower_multiplicity",
        "EcucValueCollectionModuleConfigurationLowerMultiplicityConstraint",
        "EcucValueCollection moduleConfigurationValues count must satisfy lower multiplicity",
        Severity::Error,
        "EcucValueCollection",
    );
    // 16. EcucValueCollectionModuleConfigurationUpperMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_value_collection_upper_multiplicity_eval),
        "autosar.ecuc.value_collection_module_configuration_upper_multiplicity",
        "EcucValueCollectionModuleConfigurationUpperMultiplicityConstraint",
        "EcucValueCollection moduleConfigurationValues count must satisfy upper multiplicity",
        Severity::Error,
        "EcucValueCollection",
    );
    // 17. EcucFloatParamDefUpperLimitConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_float_param_def_upper_limit_eval),
        "autosar.ecuc.float_param_def_upper_limit",
        "EcucFloatParamDefUpperLimitConstraint",
        "EcucFloatParamDef upperLimit must be a valid numerical value",
        Severity::Error,
        "EcucFloatParamDef",
    );
    // 18. ModuleConfigurationSubContainerMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_module_config_sub_container_lower_multiplicity_eval),
        "autosar.ecuc.module_configuration_sub_container_multiplicity",
        "ModuleConfigurationSubContainerMultiplicityConstraint",
        "EcucModuleConfigurationValues subContainers count must satisfy lower multiplicity",
        Severity::Error,
        "EcucModuleConfigurationValues",
    );
    // 19. ContainerSubContainerMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_sub_container_lower_multiplicity_eval),
        "autosar.ecuc.container_sub_container_multiplicity",
        "ContainerSubContainerMultiplicityConstraint",
        "EcucContainerValue subContainers count must satisfy lower multiplicity",
        Severity::Error,
        "EcucContainerValue",
    );
    // 20. GContainerParameterValueMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_parameter_value_multiplicity_eval),
        "autosar.ecuc.g_container_parameter_value_multiplicity",
        "GContainerParameterValueMultiplicityConstraint",
        "EcucContainerValue parameterValues count must satisfy lower multiplicity",
        Severity::Error,
        "EcucContainerValue",
    );
    // 21. GContainerReferenceValueMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_reference_value_multiplicity_eval),
        "autosar.ecuc.g_container_reference_value_multiplicity",
        "GContainerReferenceValueMultiplicityConstraint",
        "EcucContainerValue referenceValues count must satisfy lower multiplicity",
        Severity::Error,
        "EcucContainerValue",
    );
    // 22. GContainerDefLowerMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_def_lower_multiplicity_eval),
        "autosar.ecuc.g_container_def_lower_multiplicity",
        "GContainerDefLowerMultiplicityConstraint",
        "EcucContainerDef lowerMultiplicity must be non-negative",
        Severity::Error,
        "EcucContainerDef",
    );
    // 23. GConfigParameterLowerMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_lower_multiplicity_eval),
        "autosar.ecuc.g_config_parameter_lower_multiplicity",
        "GConfigParameterLowerMultiplicityConstraint",
        "EcucParameterDef lowerMultiplicity must be non-negative",
        Severity::Error,
        "EcucParameterDef",
    );
    // 24. GConfigParameterUpperMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_upper_multiplicity_eval),
        "autosar.ecuc.g_config_parameter_upper_multiplicity",
        "GConfigParameterUpperMultiplicityConstraint",
        "EcucParameterDef upperMultiplicity must be a valid value",
        Severity::Error,
        "EcucParameterDef",
    );
    // 25. GConfigReferenceLowerMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_abstract_reference_def_lower_multiplicity_eval),
        "autosar.ecuc.g_config_reference_lower_multiplicity",
        "GConfigReferenceLowerMultiplicityConstraint",
        "EcucAbstractReferenceDef lowerMultiplicity must be non-negative",
        Severity::Error,
        "EcucAbstractReferenceDef",
    );
    // 26. GConfigReferenceUpperMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_abstract_reference_def_upper_multiplicity_eval),
        "autosar.ecuc.g_config_reference_upper_multiplicity",
        "GConfigReferenceUpperMultiplicityConstraint",
        "EcucAbstractReferenceDef upperMultiplicity must be a valid value",
        Severity::Error,
        "EcucAbstractReferenceDef",
    );
    // 27. GParamConfContainerDefInChoiceContainerDefMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_param_conf_container_def_in_choice_multiplicity_eval),
        "autosar.ecuc.g_param_conf_container_def_in_choice_container_def_multiplicity",
        "GParamConfContainerDefInChoiceContainerDefMultiplicityConstraint",
        "EcucParamConfContainerDef lowerMultiplicity must not exceed upperMultiplicity",
        Severity::Error,
        "EcucParamConfContainerDef",
    );
    // 28. GParamConfMultiplicityConsistencyConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_param_conf_multiplicity_consistency_eval),
        "autosar.ecuc.g_param_conf_multiplicity_consistency",
        "GParamConfMultiplicityConsistencyConstraint",
        "EcucDefinitionElement lowerMultiplicity must not exceed upperMultiplicity",
        Severity::Error,
        "EcucDefinitionElement",
    );
    // 29. GModuleConfigurationChoiceContainerDefMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_module_config_choice_container_def_multiplicity_eval),
        "autosar.ecuc.g_module_configuration_choice_container_def_multiplicity",
        "GModuleConfigurationChoiceContainerDefMultiplicityConstraint",
        "EcucModuleConfigurationValues choiceContainerDef count must satisfy lower multiplicity",
        Severity::Error,
        "EcucModuleConfigurationValues",
    );
    // 30. GContainerDefUpperMultiplicityConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_def_upper_multiplicity_eval),
        "autosar.ecuc.g_container_def_upper_multiplicity",
        "GContainerDefUpperMultiplicityConstraint",
        "EcucContainerDef upperMultiplicity must be a valid value",
        Severity::Error,
        "EcucContainerDef",
    );
    // 31. EcucConfigParameterDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_default_value_valid_eval),
        "autosar.ecuc.config_parameter_default_value",
        "EcucConfigParameterDefaultValueConstraint",
        "EcucParameterDef defaultValue must be a valid value of matching type",
        Severity::Warning,
        "EcucParameterDef",
    );
    // 32. GEnumerationParamDefDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_enumeration_param_def_default_value_eval),
        "autosar.ecuc.g_enumeration_param_def_default_value",
        "GEnumerationParamDefDefaultValueConstraint",
        "EcucEnumerationParamDef defaultValue must be one of its literals",
        Severity::Error,
        "EcucEnumerationParamDef",
    );
    // 33. EcucFloatParamDefDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_float_param_def_default_value_eval),
        "autosar.ecuc.float_param_def_default_value",
        "EcucFloatParamDefDefaultValueConstraint",
        "EcucFloatParamDef defaultValue must be within [lowerLimit, upperLimit]",
        Severity::Error,
        "EcucFloatParamDef",
    );
    // 34. EcucIntegerParamDefDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_integer_param_def_default_value_eval),
        "autosar.ecuc.integer_param_def_default_value",
        "EcucIntegerParamDefDefaultValueConstraint",
        "EcucIntegerParamDef defaultValue must be within [lowerLimit, upperLimit]",
        Severity::Error,
        "EcucIntegerParamDef",
    );
    // 35. EcucLinkerSymbolDefDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_linker_symbol_def_default_value_eval),
        "autosar.ecuc.linker_symbol_def_default_value",
        "EcucLinkerSymbolDefDefaultValueConstraint",
        "EcucLinkerSymbolDef defaultValue must not be empty",
        Severity::Error,
        "EcucLinkerSymbolDef",
    );
    // 36. EcucStringParamDefDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_string_param_def_default_value_eval),
        "autosar.ecuc.string_param_def_default_value",
        "EcucStringParamDefDefaultValueConstraint",
        "EcucStringParamDef defaultValue must not be empty",
        Severity::Error,
        "EcucStringParamDef",
    );
    // 37. EcucFunctionNameDefDefaultValueConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_function_name_def_default_value_eval),
        "autosar.ecuc.function_name_def_default_value",
        "EcucFunctionNameDefDefaultValueConstraint",
        "EcucFunctionNameDef defaultValue must not be empty",
        Severity::Error,
        "EcucFunctionNameDef",
    );
    // 38. GModuleDefContainerDefinitionMissingConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_module_def_container_definition_missing_eval),
        "autosar.ecuc.g_module_def_container_definition_missing",
        "GModuleDefContainerDefinitionMissingConstraint",
        "EcucModuleDef must define at least one containerDef",
        Severity::Error,
        "EcucModuleDef",
    );
    // 39. GParamConfContainerDefConfigReferenceMissingConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_param_conf_container_def_config_reference_missing_eval),
        "autosar.ecuc.g_param_conf_container_def_config_reference_missing",
        "GParamConfContainerDefConfigReferenceMissingConstraint",
        "EcucParamConfContainerDef must define at least one reference",
        Severity::Error,
        "EcucParamConfContainerDef",
    );
    // 40. GParamConfContainerDefConfigParameterMissingConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_param_conf_container_def_config_parameter_missing_eval),
        "autosar.ecuc.g_param_conf_container_def_config_parameter_missing",
        "GParamConfContainerDefConfigParameterMissingConstraint",
        "EcucParamConfContainerDef must define at least one parameter",
        Severity::Error,
        "EcucParamConfContainerDef",
    );
    // 41. GContainerDefContainerDefinitionMissingConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_def_container_definition_missing_eval),
        "autosar.ecuc.g_container_def_container_definition_missing",
        "GContainerDefContainerDefinitionMissingConstraint",
        "EcucContainerDef must reference a valid containerDefinition",
        Severity::Error,
        "EcucContainerDef",
    );
    // 42. EcucIntegerParamDefLowerLimitConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_integer_param_def_lower_limit_eval),
        "autosar.ecuc.integer_param_def_lower_limit",
        "EcucIntegerParamDefLowerLimitConstraint",
        "EcucIntegerParamDef lowerLimit must be a valid numerical value",
        Severity::Error,
        "EcucIntegerParamDef",
    );
    // 43. EcucIntegerParamDefUpperLimitConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_integer_param_def_upper_limit_eval),
        "autosar.ecuc.integer_param_def_upper_limit",
        "EcucIntegerParamDefUpperLimitConstraint",
        "EcucIntegerParamDef upperLimit must be a valid numerical value",
        Severity::Error,
        "EcucIntegerParamDef",
    );
    // 44. EcucParameterDefSymbolicNameValueModifyConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_symbolic_name_modify_eval),
        "autosar.ecuc.parameter_def_symbolic_name_value_modify",
        "EcucParameterDefSymbolicNameValueModifyConstraint",
        "EcucParameterDef symbolicName must conform to naming rules when modified",
        Severity::Warning,
        "EcucParameterDef",
    );
    // 45. EcucContainerDefPostBuildChangeableModifyConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_container_def_post_build_changeable_modify_eval),
        "autosar.ecuc.container_def_post_build_changeable_modify",
        "EcucContainerDefPostBuildChangeableModifyConstraint",
        "EcucContainerDef postBuildChangeable must remain consistent when modified",
        Severity::Warning,
        "EcucContainerDef",
    );
    // 46. EcucImplementationConfigurationClassLinkTimeConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_implementation_config_class_link_time_eval),
        "autosar.ecuc.implementation_configuration_class_link_time",
        "EcucImplementationConfigurationClassLinkTimeConstraint",
        "EcucImplementationConfigurationClass LinkTime constraints must hold",
        Severity::Error,
        "EcucImplementationConfigurationClass",
    );
    // 47. EcucImplementationConfigurationClassPreCompileConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_implementation_config_class_pre_compile_eval),
        "autosar.ecuc.implementation_configuration_class_pre_compile",
        "EcucImplementationConfigurationClassPreCompileConstraint",
        "EcucImplementationConfigurationClass PreCompile constraints must hold",
        Severity::Error,
        "EcucImplementationConfigurationClass",
    );
    // 48. EcucParamConfContainerDefMultipleConfigurationModifyConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_param_conf_container_def_multiple_configuration_modify_eval),
        "autosar.ecuc.param_conf_container_def_multiple_configuration_modify",
        "EcucParamConfContainerDefMultipleConfigurationModifyConstraint",
        "EcucParamConfContainerDef multipleConfiguration must keep multiplicity consistent",
        Severity::Warning,
        "EcucParamConfContainerDef",
    );
    // 49. EcucParameterDefImplConfigClassConstraint
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_impl_config_class_eval),
        "autosar.ecuc.parameter_def_impl_config_class",
        "EcucParameterDefImplConfigClassConstraint",
        "EcucParameterDef implementationConfigClass must be a recognized value when present",
        Severity::Warning,
        "EcucParameterDef",
    );
}
