//! JET-subset template engine tests.
//!
//! A verbatim port of C++ `emf-ecore-codegen`'s:
//! - `P4_JetTemplateTests.cpp` (12 tests: each / if / unless / nesting)
//! - the `renderTemplate` cases (basicSubstitution, unknownKeptAsIs) from
//!   `CppTemplatesTests.cpp`
//!
//! Template strings and expected outputs are copy-pasted from the C++ test
//! inputs so the port can be checked line-for-line against the reference.

use std::collections::HashMap;

use emf_ecore_codegen::template::{render_jet_template, render_template};

type Vars = HashMap<String, String>;
type Row = HashMap<String, String>;
type Lists = HashMap<String, Vec<Row>>;

fn vars(pairs: &[(&str, &str)]) -> Vars {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn row(pairs: &[(&str, &str)]) -> Row {
    vars(pairs)
}

/// Render JET with abstract `vars`/`lists` (Rust-side stand-in for C++'s
/// `std::map<std::string, std::string>` / list-of-maps).
fn jet(tmpl: &str, v: &Vars, l: &Lists) -> String {
    render_jet_template(tmpl, v, l)
}

// =====================================================================
// 1) Plain (no control blocks) behaves exactly like renderTemplate
// =====================================================================
#[test]
fn jet_template_plain_no_control_blocks_same_as_render_template() {
    let tmpl = "Hello {{name}}, age {{age}}!";
    let v = vars(&[("name", "alice"), ("age", "30")]);
    let a = render_template(tmpl, &v);
    let b = jet(tmpl, &v, &Lists::new());
    assert_eq!(a, b);
    assert_eq!(a, "Hello alice, age 30!");
}

// =====================================================================
// 2) each: expands every row of a list
// =====================================================================
#[test]
fn jet_template_each_renders_all_rows() {
    let tmpl = "names:\n\
{{#each people}}\
  - {{name}} (age {{age}})\n\
{{/each}}\
end";
    let l = Lists::from([(
        "people".to_string(),
        vec![
            row(&[("name", "alice"), ("age", "30")]),
            row(&[("name", "bob"), ("age", "25")]),
            row(&[("name", "carol"), ("age", "28")]),
        ],
    )]);
    let out = jet(tmpl, &Vars::new(), &l);
    assert!(out.contains("- alice (age 30)"), "out was: {out}");
    assert!(out.contains("- bob (age 25)"), "out was: {out}");
    assert!(out.contains("- carol (age 28)"), "out was: {out}");
}

// =====================================================================
// 3) each + outer var: an each body can reference outer vars
// =====================================================================
#[test]
fn jet_template_each_inherits_outer_vars() {
    let tmpl = "package: {{pkg}}\n\
{{#each classes}}\
class: {{name}} in {{pkg}}\n\
{{/each}}";
    let v = vars(&[("pkg", "com.example")]);
    let l = Lists::from([(
        "classes".to_string(),
        vec![row(&[("name", "Foo")]), row(&[("name", "Bar")])],
    )]);
    let out = jet(tmpl, &v, &l);
    assert!(out.contains("package: com.example"), "out was: {out}");
    assert!(out.contains("class: Foo in com.example"), "out was: {out}");
    assert!(out.contains("class: Bar in com.example"), "out was: {out}");
}

// =====================================================================
// 4) each: a row field shadows an outer var inside the block
// =====================================================================
#[test]
fn jet_template_each_row_overrides_outer() {
    let tmpl = "{{#each rows}}{{x}};{{/each}}";
    let v = vars(&[("x", "OUTER")]);
    let l = Lists::from([(
        "rows".to_string(),
        vec![row(&[("x", "A")]), row(&[("x", "B")]), row(&[("x", "C")])],
    )]);
    let out = jet(tmpl, &v, &l);
    assert_eq!(out, "A;B;C;");
}

// =====================================================================
// 5) each with an empty (or missing) list produces an empty block
// =====================================================================
#[test]
fn jet_template_each_empty_list_produces_empty_block() {
    let tmpl = "before{{#each items}}<{{name}}>{{/each}}after";
    let out = jet(tmpl, &Vars::new(), &Lists::new()); // items missing
    assert_eq!(out, "beforeafter");
}

// =====================================================================
// 6) if: renders when cond is present and non-empty
// =====================================================================
#[test]
fn jet_template_if_condition_true_renders() {
    let tmpl = "before{{#if flag}}ENABLED{{/if}}after";
    let v = vars(&[("flag", "yes")]);
    let out = jet(tmpl, &v, &Lists::new());
    assert_eq!(out, "beforeENABLEDafter");
}

// =====================================================================
// 7) if: cond missing -> block is skipped
// =====================================================================
#[test]
fn jet_template_if_condition_missing_skips() {
    let tmpl = "before{{#if flag}}ENABLED{{/if}}after";
    let out = jet(tmpl, &Vars::new(), &Lists::new()); // flag missing
    assert_eq!(out, "beforeafter");
}

// =====================================================================
// 8) if: cond being an empty string is falsey
// =====================================================================
#[test]
fn jet_template_if_empty_string_falsey() {
    let tmpl = "[{{#if flag}}X{{/if}}]";
    let v = vars(&[("flag", "")]); // "" == false
    let out = jet(tmpl, &v, &Lists::new());
    assert_eq!(out, "[]");
}

// =====================================================================
// 9) unless: cond present+non-empty -> skipped
// =====================================================================
#[test]
fn jet_template_unless_condition_true_skips() {
    let tmpl = "before{{#unless flag}}DISABLED{{/unless}}after";
    let v = vars(&[("flag", "yes")]);
    let out = jet(tmpl, &v, &Lists::new());
    assert_eq!(out, "beforeafter");
}

// =====================================================================
// 10) unless: cond missing -> renders
// =====================================================================
#[test]
fn jet_template_unless_condition_missing_renders() {
    let tmpl = "before{{#unless flag}}DISABLED{{/unless}}after";
    let out = jet(tmpl, &Vars::new(), &Lists::new()); // flag missing
    assert_eq!(out, "beforeDISABLEDafter");
}

// =====================================================================
// 11) Nested each: an each inside an each (inner list looked up in the
//     top-level `lists` map; C++ triggers via a top-level list, so the row
//     does not need to carry the inner list).
// =====================================================================
#[test]
fn jet_template_nested_each() {
    let tmpl = "{{#each classes}}\
class {{name}}:\n\
{{#each features}}\
  - {{name}} ({{type}})\n\
{{/each}}\
{{/each}}";
    let mut l = Lists::new();
    // C++ supplies only one class row; the inner `features` list is absent so
    // the nested each emits nothing, but the outer "class Foo:" line renders.
    l.insert("classes".to_string(), vec![row(&[("name", "Foo"), ("features", "")])]);
    let out = jet(tmpl, &Vars::new(), &l);
    assert!(out.contains("class Foo:"), "out was: {out}");
}

// =====================================================================
// 12) if inside each: per-row truthiness decides the starred entries
// =====================================================================
#[test]
fn jet_template_if_inside_each() {
    let tmpl = "{{#each items}}\
{{#if active}}*{{name}};{{/if}}\
{{name}};\
{{/each}}";
    let l = Lists::from([(
        "items".to_string(),
        vec![
            row(&[("name", "A"), ("active", "yes")]),
            row(&[("name", "B"), ("active", "")]), // inactive
            row(&[("name", "C"), ("active", "true")]), // active
        ],
    )]);
    let out = jet(tmpl, &Vars::new(), &l);
    assert_eq!(out, "*A;A;B;*C;C;");
}

// =====================================================================
// renderTemplate (from CppTemplatesTests.cpp)
// =====================================================================
/// 1. basic substitution of known placeholders.
#[test]
fn render_template_basic_substitution() {
    let tpl = "Hello {{name}}, you are {{age}} years old.";
    let v = vars(&[("name", "Alice"), ("age", "30")]);
    let out = render_template(tpl, &v);
    assert_eq!(out, "Hello Alice, you are 30 years old.");
}

/// 2. unknown placeholders are kept as-is.
#[test]
fn render_template_unknown_kept_as_is() {
    let tpl = "a={{a}}, b={{b}}, c={{c}}";
    let v = vars(&[("a", "1")]);
    let out = render_template(tpl, &v);
    assert!(out.contains("a=1"), "out was: {out}");
    assert!(out.contains("{{b}}"), "out was: {out}");
    assert!(out.contains("{{c}}"), "out was: {out}");
}