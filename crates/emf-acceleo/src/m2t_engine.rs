//! Model-to-Text rendering engine.
//!
//! Given a parsed [`TemplateFile`] and a [`ValueContext`], the engine picks the
//! first public parameterless template as the entry point and renders its body.
//! Bodies support:
//!
//! - `[$name/]` variable substitution (via [`ValueContext`]).
//! - `[if $cond]...[else/]...[/if]` conditionals, where a non-empty bound value
//!   is truthy.
//! - `[for $it : $list]...</for>` iteration over a space-separated list.
//!
//! Anything that is not recognized is copied through verbatim, so plain text
//! and unknown tags survive untouched.

use crate::template::{strip_comments, substitute, DeclKind, TemplateFile, ValueContext};

/// An error raised while rendering a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M2tError {
    /// Human-readable message.
    pub message: String,
}

impl std::fmt::Display for M2tError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for M2tError {}

impl M2tError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

/// Render a template file with the given context.
///
/// Uses the first public, parameterless entry point as the root template.
pub fn render(file: &TemplateFile, ctx: &ValueContext) -> Result<String, M2tError> {
    let entry = file
        .entry_points()
        .into_iter()
        .next()
        .ok_or_else(|| M2tError::new("no public entry-point template found"))?;
    render_template(entry, ctx, file)
}

/// Render a specific named template.
pub fn render_named(
    file: &TemplateFile,
    name: &str,
    ctx: &ValueContext,
) -> Result<String, M2tError> {
    let decl = file
        .get(name)
        .ok_or_else(|| M2tError::new(format!("unknown template `{name}`")))?;
    if decl.kind != DeclKind::Template {
        return Err(M2tError::new(format!("`{name}` is not a template")));
    }
    render_template(decl, ctx, file)
}

/// Render a single template declaration body.
fn render_template(
    decl: &crate::template::TemplateDecl,
    ctx: &ValueContext,
    file: &TemplateFile,
) -> Result<String, M2tError> {
    let body = strip_comments(&decl.body);
    render_block(&body, ctx, file)
}

/// Render a block of mixed text and tags.
fn render_block(body: &str, ctx: &ValueContext, file: &TemplateFile) -> Result<String, M2tError> {
    let mut out = String::new();
    let mut rest = body;
    while let Some(pos) = rest.find("[") {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let close = match rest.find(']') {
            Some(c) => c,
            None => {
                out.push_str(rest);
                break;
            }
        };
        let tag = rest[..=close].to_string();
        let inner = rest[1..close].trim();
        if let Some(remaining) = inner.strip_prefix("if ") {
            out.push_str(&render_if(
                remaining, &tag, rest, close, ctx, file, &mut rest,
            )?);
            continue;
        }
        if let Some(for_rest) = inner.strip_prefix("for ") {
            out.push_str(&render_for(
                for_rest, &tag, rest, close, ctx, file, &mut rest,
            )?);
            continue;
        }
        if inner == "else" || inner == "/if" || inner == "/for" {
            // Handled by the enclosing construct; copy nothing here.
            out.push_str(&tag);
            rest = &rest[close + 1..];
            continue;
        }
        // Plain `$var` / `$var/` reference, or an unrecognized tag.
        if let Some(key) = inner.trim_end_matches('/').strip_prefix('$') {
            if let Some(v) = ctx.get(key.trim()) {
                out.push_str(v);
            }
        } else {
            out.push_str(&substitute(&tag, ctx));
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn render_if<'a>(
    condition: &str,
    _tag: &str,
    full: &'a str,
    open_close: usize,
    ctx: &ValueContext,
    file: &TemplateFile,
    rest: &mut &'a str,
) -> Result<String, M2tError> {
    let cond = condition.trim();
    let truthy = if let Some(key) = cond.strip_prefix('$') {
        ctx.get(key.trim()).map(|v| !v.is_empty()).unwrap_or(false)
    } else {
        !cond.is_empty()
    };
    // body runs from after `]` to `[/if]`.
    let body_src = &full[open_close + 1..];
    let endif_rel = body_src
        .find("[/if]")
        .ok_or_else(|| M2tError::new("missing `[/if]`"))?;
    let (then_end, else_start, else_end);
    match body_src[..endif_rel].find("[else") {
        Some(e) => {
            let marker_end = body_src[e..endif_rel]
                .find(']')
                .map(|m| e + m + 1)
                .unwrap_or(endif_rel);
            then_end = e;
            else_start = marker_end;
            else_end = endif_rel;
        }
        None => {
            then_end = endif_rel;
            else_start = endif_rel;
            else_end = endif_rel;
        }
    }
    let then_text = &body_src[..then_end];
    let else_text = &body_src[else_start..else_end];
    *rest = &body_src[endif_rel + "[/if]".len()..];
    if truthy {
        render_block(then_text, ctx, file)
    } else {
        render_block(else_text, ctx, file)
    }
}

fn render_for<'a>(
    header: &str,
    _tag: &str,
    full: &'a str,
    open_close: usize,
    ctx: &ValueContext,
    file: &TemplateFile,
    rest: &mut &'a str,
) -> Result<String, M2tError> {
    // header: `$item : $list`
    let mut it = header.trim().split(':');
    let item = it
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('$')
        .to_string();
    let list_name = it
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('$')
        .to_string();
    let list = match ctx.get(&list_name) {
        Some(l) => l,
        None => {
            // consume through [/for]
            let body_src = &full[open_close + 1..];
            let end = body_src.find("[/for]").unwrap_or(body_src.len());
            *rest = &body_src[end + "[/for]".len()..];
            return Ok(String::new());
        }
    };
    let items: Vec<&str> = list.split_whitespace().collect();
    let body_src = &full[open_close + 1..];
    let end = body_src.find("[/for]").unwrap_or(body_src.len());
    let body = &body_src[..end];
    *rest = &body_src[end + "[/for]".len()..];
    let mut out = String::new();
    for value in items {
        let mut child = ctx.clone();
        child.bind(&item, value.to_string());
        out.push_str(&render_block(body, &child, file)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_entry_point() {
        let src = "[template public root()]\nHello [$name/]\n[/template]";
        let file = crate::mtl_parser::parse_templates(src).expect("parse");
        let mut ctx = ValueContext::new();
        ctx.bind("name", "world");
        assert_eq!(render(&file, &ctx).unwrap(), "\nHello world\n");
    }

    #[test]
    fn renders_condition() {
        let src = "[template public root()][if $flag]YES[else/]NO[/if][/template]";
        let file = crate::mtl_parser::parse_templates(src).expect("parse");
        let mut ctx = ValueContext::new();
        ctx.bind("flag", "1");
        assert_eq!(render(&file, &ctx).unwrap(), "YES");
        ctx.bind("flag", "");
        assert_eq!(render(&file, &ctx).unwrap(), "NO");
    }

    #[test]
    fn renders_loop() {
        let src = "[template public root()][for $x : $items]([$x]);
[/for][/template]";
        let file = crate::mtl_parser::parse_templates(src).expect("parse");
        let mut ctx = ValueContext::new();
        ctx.bind("items", "a b c");
        let out = render(&file, &ctx).unwrap();
        assert!(out.contains("(a);"));
        assert!(out.contains("(c);"));
    }

    #[test]
    fn renders_named_template() {
        let src = "[template public a()]AA[/template]";
        let file = crate::mtl_parser::parse_templates(src).expect("parse");
        let ctx = ValueContext::new();
        assert_eq!(render_named(&file, "a", &ctx).unwrap(), "AA");
    }

    #[test]
    fn missing_entry_is_error() {
        let file = TemplateFile::default();
        let ctx = ValueContext::new();
        assert!(render(&file, &ctx).is_err());
    }
}
