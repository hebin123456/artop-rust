//! Language-agnostic template rendering engine.
//!
//! A faithful port of the *pure rendering* half of C++ `emf-ecore-codegen`'s
//! `CppTemplates.cpp` — the `renderTemplate` / `renderJetTemplate` functions.
//! The C++ `emit*` family (which renders C++ source text for classes,
//! factories, packages, ...) is intentionally **not** ported: the Rust crate
//! already has its own generator emitting Rust source instead.
//!
//! This module only implements the string/template semantics, which are
//! language independent:
//!
//! - `render_template`: replace `{{name}}` placeholders from a `vars` map.
//!   Unknown placeholders are left as-is (`{{name}}`).
//! - `render_jet_template`: on top of placeholder substitution, expands a JET
//!   subset of control blocks:
//!   - `{{#each list}}...{{/each}}`      iterate a list of row-vars
//!   - `{{#if cond}}...{{/if}}`          render when `cond` exists and non-empty
//!   - `{{#unless cond}}...{{/unless}}`  render when `cond` is absent or empty
//!
//! Semantics mirror the C++ implementation exactly (including its parsing
//! order and truthy rules) so the ported JET tests reproduce verbatim.

use std::collections::HashMap;

/// A single matched control block: `{{#TYPE key}} ... {{/TYPE}}`.
#[derive(Default)]
struct BlockMatch {
    /// `"each"` / `"if"` / `"unless"`.
    ty: String,
    /// The block argument, e.g. `"features"` for `{{#each features}}`.
    key: String,
    /// Byte index just past the opening `}}`.
    open_end: usize,
    /// Byte index where the block body begins (== `open_end`).
    body_start: usize,
    /// Byte index where the closing `{{/TYPE}}` begins.
    close_start: usize,
    /// Byte index just past the closing `{{/TYPE}}`.
    close_end: usize,
}

/// Find the first non-nested control block at or after byte index `start`.
///
/// Mirrors C++ `findFirstControlBlock`: scan forward for the first `{{#`, parse
/// the `TYPE key` header, then locate the matching `{{/TYPE}}` closer. The
/// outermost block is returned; nested blocks are handled lazily by recursive
/// re-rendering of the body (so an `each` inside an `each` works).
fn find_first_control_block(s: &str, start: usize) -> BlockMatch {
    let len = s.len();
    let mut pos = start;
    while pos < len {
        let Some(rel_open) = s[pos..].find("{{#") else {
            break;
        };
        let open_brace = pos + rel_open;
        let after_open = open_brace + 3;
        if after_open >= len {
            break;
        }
        let Some(rel_close_open) = s[after_open..].find("}}") else {
            break;
        };
        let close_open_brace = after_open + rel_close_open;
        let header = &s[after_open..close_open_brace];
        let Some(sp) = header.find(' ') else {
            // Not a `TYPE key` header; skip this `{{#...}}` and keep scanning.
            pos = close_open_brace + 2;
            continue;
        };
        let ty = &header[..sp];
        if ty != "each" && ty != "if" && ty != "unless" {
            pos = close_open_brace + 2;
            continue;
        }
        let key = &header[sp + 1..];
        // Yields the literal `{{/TYPE}}` (four doubled braces encode two braces).
        let close_marker = format!("{{{{/{ty}}}}}");
        let Some(rel_close) = s[(close_open_brace + 2)..].find(&close_marker) else {
            break;
        };
        let close_pos = close_open_brace + 2 + rel_close;
        let mut m = BlockMatch::default();
        m.ty = ty.to_string();
        m.key = key.to_string();
        m.open_end = close_open_brace + 2;
        m.body_start = close_open_brace + 2;
        m.close_start = close_pos;
        m.close_end = close_pos + close_marker.len();
        return m;
    }
    BlockMatch::default()
}

/// Substitute `{{name}}` placeholders in `text` from `vars`.
///
/// Mirrors the C++ single-line replace: one left-to-right pass. `{{key}}` is
/// replaced by `vars[key]` when present; otherwise the placeholder literal
/// (`{{key}}`) is preserved.
fn substitute_vars(text: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let len = text.len();
    while i < len {
        let b = text.as_bytes()[i];
        if b == b'{' && i + 1 < len && text.as_bytes()[i + 1] == b'{' {
            if let Some(rel) = text[i + 2..].find("}}") {
                let end = i + 2 + rel;
                let key = &text[i + 2..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push('{');
                        out.push_str(key);
                        out.push('}');
                        out.push('}');
                    }
                }
                i = end + 2;
            } else {
                out.push('{');
                i += 1;
            }
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Replace `{{name}}` placeholders in `tmpl`. Unknown placeholders are kept
/// verbatim. Equivalent to C++ `renderTemplate` (which delegates to
/// `renderJetTemplate` with an empty `lists` map).
pub fn render_template(tmpl: &str, vars: &HashMap<String, String>) -> String {
    render_jet_template(tmpl, vars, &HashMap::new())
}

/// Render `tmpl`, expanding the JET subset of control blocks.
///
/// `vars` provides the outer variable scope; `lists` maps each `{{#each name}}`
/// list argument to the rows to iterate over. Truthiness for `{{#if}}` /
/// `{{#unless}}` follows C++: a variable counts as true when it *exists* in
/// `vars` and its value is a non-empty string; otherwise false.
///
/// `{{#each}}` rows inherit the outer `vars` and may shadow them (a row field
/// takes precedence inside the row's body — the C++ `merged = vars + row`
/// merge where the row wins). Bodies are rendered by recursive expansion from
/// the outermost block inward, so one level of `each`-inside-`each` is
/// supported (and `if`/`unless` render bodies the same way).
pub fn render_jet_template(
    tmpl: &str,
    vars: &HashMap<String, String>,
    lists: &HashMap<String, Vec<HashMap<String, String>>>,
) -> String {
    let mut out = String::with_capacity(tmpl.len());
    let mut pos = 0usize;
    while pos < tmpl.len() {
        let bm = find_first_control_block(tmpl, pos);
        if bm.ty.is_empty() {
            // No control block left: copy the remainder with `{{var}}` replacement.
            out.push_str(&substitute_vars(&tmpl[pos..], vars));
            break;
        }

        // 1) Copy the static text before this block (with placeholder substitution).
        let open_brace_loc = tmpl[..bm.open_end].rfind("{{#").unwrap_or(bm.open_end);
        out.push_str(&substitute_vars(&tmpl[pos..open_brace_loc], vars));

        // 2) Render the block body.
        let body = &tmpl[bm.body_start..bm.close_start];
        match bm.ty.as_str() {
            "each" => {
                if let Some(rows) = lists.get(&bm.key) {
                    for row in rows {
                        // Merge outer vars + row (row shadows outer on conflict),
                        // then recursively render the body in the merged scope.
                        let mut merged: HashMap<String, String> = vars.clone();
                        for (k, v) in row {
                            merged.insert(k.clone(), v.clone());
                        }
                        out.push_str(&render_jet_template(body, &merged, lists));
                    }
                }
            }
            "if" => {
                let cond = vars.get(&bm.key).map(|v| !v.is_empty()).unwrap_or(false);
                if cond {
                    out.push_str(&render_jet_template(body, vars, lists));
                }
            }
            "unless" => {
                let cond = vars.get(&bm.key).map(|v| !v.is_empty()).unwrap_or(false);
                if !cond {
                    out.push_str(&render_jet_template(body, vars, lists));
                }
            }
            _ => unreachable!("control block type validated in find_first_control_block"),
        }

        // 3) Skip past the closing `{{/TYPE}}`.
        pos = bm.close_end;
    }
    out
}