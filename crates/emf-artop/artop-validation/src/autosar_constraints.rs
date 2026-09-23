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
//! - `autosar.uuid_globally_unique` — model-level UUID uniqueness, DFS whole tree (BATCH)
//!
//! Plus `register_ecuc_constraints` (a representative subset of the C++ 49 ECUC
//! constraints) and the standalone [`validate_uuid_uniqueness`] tree sweep.

use emf_common::diagnostic::{Diagnostic, Severity as CommonSeverity};
use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;
use emf_validation::constraint::{Constraint, ConstraintMode, Severity};
use emf_validation::diagnostician::map_severity;
use emf_validation::e_validator::EValidator;
use std::collections::{HashMap, HashSet};

// ===================== reflective helpers =====================

/// Read a string feature; `None` when the feature is absent or not a string.
fn read_str(obj: &dyn EObject, name: &str) -> Option<String> {
    obj.e_get(name).and_then(|v| v.as_str().map(String::from))
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
/// NOTE (trade-off vs C++): the C++ evaluator keys on the feature's
/// `lowerBound >= 1`. The `EObject` trait exposes no feature-metadata surface
/// for that, so -- being model-technical agnostic -- we approximate "required"
/// as "the `category` feature is present and set to an empty string". The test
/// model's `category` uses `lowerBound = 1`, so the behaviour coincides there;
/// a genuinely optional `category` would need the ecore metadata layer.
fn category_required_eval(obj: &dyn EObject) -> bool {
    match obj.e_get("category") {
        Some(val) => match val.as_str() {
            Some(s) => !s.is_empty(),
            None => true,
        },
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

/// Record one object's `uuid` into the global-uniqueness sweep.
fn check_uuid(seen: &mut HashSet<String>, ok: &mut bool, obj: &dyn EObject) {
    let Some(val) = obj.e_get("uuid") else {
        return;
    };
    let Some(u) = val.as_str() else {
        return;
    };
    if u.is_empty() {
        *ok = false;
    } else if !seen.insert(u.to_string()) {
        *ok = false;
    }
}

/// `false` when the subtree rooted at `root` (inclusive) contains an empty uuid
/// or two objects sharing one non-empty uuid.
fn subtree_uuids_valid(root: &dyn EObject) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut ok = true;
    check_uuid(&mut seen, &mut ok, root);
    let mut stack: Vec<ObjectRef> = root.e_contents();
    while let Some(child) = stack.pop() {
        let c = child.borrow();
        check_uuid(&mut seen, &mut ok, &*c);
        for g in c.e_contents() {
            stack.push(g);
        }
    }
    ok
}

fn uuid_globally_unique_eval(obj: &dyn EObject) -> bool {
    subtree_uuids_valid(obj)
}

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
        if let Some(val) = obj.e_get("uuid") {
            if let Some(u) = val.as_str() {
                if u.is_empty() {
                    result.push(Diagnostic::new(
                        CommonSeverity::Error,
                        "AutosarUuidGloballyUnique",
                        0,
                        "AUTOSAR Identifiable.uuid must not be empty",
                    ));
                } else if let Some(&first) = first_owners.get(u) {
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
                    first_owners.insert(u.to_string(), next);
                }
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

    // uuid global uniqueness (model-level, DFS full tree): BATCH
    validator.add_constraint(
        Box::new(uuid_globally_unique_eval),
        "autosar.uuid_globally_unique",
        "AutosarUuidGloballyUnique",
        "AUTOSAR Identifiable.uuid must be globally unique",
        Severity::Error,
        ConstraintMode::Batch,
    );
}

// ===================== ECUC constraints (representative subset) =====================

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

// ---- evaluators (align C++ names, robust reflective semantics) ----

fn ecuc_container_value_has_definition_eval(obj: &dyn EObject) -> bool {
    read_object_ref(obj, "definition").is_some()
}

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

fn ecuc_textual_param_value_non_empty_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "value") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

fn ecuc_definition_element_multiplicity_basic_eval(obj: &dyn EObject) -> bool {
    match read_numeric(obj, "lowerMultiplicity") {
        Some(l) => l >= 0.0,
        None => true,
    }
}

fn ecuc_reference_def_has_destination_eval(obj: &dyn EObject) -> bool {
    read_object_ref(obj, "destination").is_some()
}

fn ecuc_enumeration_param_def_has_literals_eval(obj: &dyn EObject) -> bool {
    ref_list_size(obj, "literals") > 0
}

fn ecuc_parameter_def_symbolic_name_non_empty_eval(obj: &dyn EObject) -> bool {
    match read_str(obj, "symbolicName") {
        Some(s) => !s.is_empty(),
        None => true,
    }
}

/// Register representative ECUC constraints (a robust subset of the C++ 49),
/// each scoped by `clientContext`-style target class name.
pub fn register_ecuc_constraints(validator: &mut EValidator) {
    register_ecuc(
        validator,
        Box::new(ecuc_container_value_has_definition_eval),
        "autosar.ecuc.g_container_basic",
        "GContainerBasicConstraint",
        "EcucContainerValue must reference a valid ContainerDef definition",
        Severity::Error,
        "EcucContainerValue",
    );
    register_ecuc(
        validator,
        Box::new(ecuc_numerical_param_value_within_limits_eval),
        "autosar.ecuc.numerical_param_value_basic",
        "EcucNumericalParamValueBasicConstraint",
        "EcucNumericalParamValue value must be within ParameterDef lower/upper limits",
        Severity::Error,
        "EcucNumericalParamValue",
    );
    register_ecuc(
        validator,
        Box::new(ecuc_textual_param_value_non_empty_eval),
        "autosar.ecuc.textual_param_value_basic",
        "EcucTextualParamValueBasicConstraint",
        "EcucTextualParamValue value must not be empty",
        Severity::Error,
        "EcucTextualParamValue",
    );
    register_ecuc(
        validator,
        Box::new(ecuc_definition_element_multiplicity_basic_eval),
        "autosar.ecuc.g_param_conf_multiplicity_basic",
        "GParamConfMultiplicityBasicConstraint",
        "EcucDefinitionElement lowerMultiplicity must be a non-negative value",
        Severity::Error,
        "EcucDefinitionElement",
    );
    register_ecuc(
        validator,
        Box::new(ecuc_reference_def_has_destination_eval),
        "autosar.ecuc.g_reference_def_basic",
        "GReferenceDefBasicConstraint",
        "EcucReferenceDef must reference a valid destination",
        Severity::Error,
        "EcucReferenceDef",
    );
    register_ecuc(
        validator,
        Box::new(ecuc_enumeration_param_def_has_literals_eval),
        "autosar.ecuc.g_enumeration_param_def_enumeration_literal",
        "GEnumerationParamDefEnumerationLiteralConstraint",
        "EcucEnumerationParamDef must define at least one literal",
        Severity::Warning,
        "EcucEnumerationParamDef",
    );
    register_ecuc(
        validator,
        Box::new(ecuc_parameter_def_symbolic_name_non_empty_eval),
        "autosar.ecuc.g_config_parameter_symbolic_name_value",
        "GConfigParameterSymbolicNameValueConstraint",
        "EcucParameterDef symbolicName must not be empty when present",
        Severity::Error,
        "EcucParameterDef",
    );
}