//! Parser for Acceleo MTL template files.
//!
//! Supports the common declaration syntax:
//!
//! ```mtl
//! [comment encoding = UTF-8 /]
//! [module bms('/example')/]
//! [import other::file /]
//! [template public genClass(a : EClass)]
//!   Hello [$a.name/]
//! [/template]
//! [query public name(e : EObject) : String = e.name/]
//! ```
//!
//! The parser extracts template and query declarations together with their
//! formal parameters and raw bodies. It intentionally keeps bodies opaque:
//! interpretation happens at render time in the [`m2t_engine`].

use crate::template::{DeclKind, TemplateDecl, TemplateFile, TemplateParam, Visibility};

/// A single error produced while scanning an MTL file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParseError {
    /// One-based line.
    pub line: usize,
    /// Message.
    pub message: String,
}

impl std::fmt::Display for TemplateParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TemplateParseError {}

/// Parse template source text into a [`TemplateFile`].
pub fn parse_templates(src: &str) -> Result<TemplateFile, TemplateParseError> {
    let mut file = TemplateFile::default();
    let mut rest = src;
    while let Some(pos) = rest.find("[") {
        rest = &rest[pos..];
        let end = match rest.find(']') {
            Some(e) => e,
            None => break,
        };
        let header = &rest[1..end];
        let first = header.split_whitespace().next().unwrap_or("");
        match first {
            "template" | "query" => {
                let (decl, consumed) = parse_decl(rest)?;
                file.push(decl);
                rest = &rest[consumed..];
            }
            _ => {
                // Module/import/comment/protected tags: skip this `[...]`.
                rest = &rest[end + 1..];
            }
        }
    }
    Ok(file)
}

/// Parse (and consume) a single `[...]` declaration starting at `src`.
fn parse_decl(src: &str) -> Result<(TemplateDecl, usize), TemplateParseError> {
    let end_open = src.find(']').unwrap_or(src.len());
    let header = &src[1..end_open];
    let mut parts = header.split_whitespace();
    let kind_word = parts.next().unwrap_or("").to_string();
    let kind = match kind_word.as_str() {
        "template" => DeclKind::Template,
        "query" => DeclKind::Query,
        other => {
            return Err(TemplateParseError {
                line: 1,
                message: format!("unexpected declaration kind `{other}`"),
            })
        }
    };
    // shape: <kind> [visibility] name ( params ) [= expr]
    let mut rest_hdr = skip_word(header, &kind_word);
    let mut visibility = Visibility::Public;
    if let Some(v) = rest_hdr.split_whitespace().next() {
        match v {
            "public" => {
                visibility = Visibility::Public;
                rest_hdr = skip_word(rest_hdr, "public");
            }
            "private" => {
                visibility = Visibility::Private;
                rest_hdr = skip_word(rest_hdr, "private");
            }
            "protected" => {
                visibility = Visibility::Protected;
                rest_hdr = skip_word(rest_hdr, "protected");
            }
            _ => {}
        }
    }
    let name = read_word(rest_hdr);
    rest_hdr = skip_word(rest_hdr, &name);
    let mut params = Vec::new();
    let trimmed = rest_hdr.trim_start();
    if trimmed.starts_with('(') {
        if let Some(close_rel) = trimmed.find(')') {
            params = parse_params(&trimmed[1..close_rel]);
            rest_hdr = trimmed[close_rel + 1..].trim_start();
        }
    }

    // Body handling.
    let consumed;
    let body;
    match kind {
        DeclKind::Template => {
            let body_start = end_open + 1;
            match find_closing(src, body_start) {
                Some((body_before, body_consumed)) => {
                    body = body_before.to_string();
                    consumed = body_start + body_consumed;
                }
                None => {
                    return Err(TemplateParseError {
                        line: 1,
                        message: "missing closing [/template]".into(),
                    })
                }
            }
        }
        DeclKind::Query => {
            let expr = if let Some(eq) = rest_hdr.find('=') {
                rest_hdr[eq + 1..].trim().to_string()
            } else {
                rest_hdr.trim().to_string()
            };
            body = expr;
            consumed = end_open + 1;
        }
    }
    let line = src[..consumed].split('\n').count();
    Ok((
        TemplateDecl {
            kind,
            name,
            visibility,
            params,
            body,
            line,
        },
        consumed,
    ))
}

/// Split a comma-separated parameter list into typed params.
fn parse_params(s: &str) -> Vec<TemplateParam> {
    s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (name, ty) = match p.split_once(':') {
                Some((n, t)) => (n.trim().to_string(), t.trim().to_string()),
                None => (p.to_string(), String::new()),
            };
            if name.is_empty() {
                None
            } else {
                Some(TemplateParam::new(name, ty))
            }
        })
        .collect()
}

/// Read the first whitespace/(-delimited word from `src`.
fn read_word(src: &str) -> String {
    src.trim_start()
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '(')
        .collect()
}

/// Skip a leading whitespace-delimited word if it equals `word`.
fn skip_word<'a>(src: &'a str, word: &str) -> &'a str {
    let s = src.trim_start();
    if read_word(s) == word {
        &s[word.len()..]
    } else {
        s
    }
}

/// Find the closing `[/template]` marker starting at `start`.
/// Returns `(body_text, len_after_marker)`.
fn find_closing(src: &str, start: usize) -> Option<(&str, usize)> {
    let marker = "[/template]";
    let idx = src[start..].find(marker)?;
    let body = &src[start..start + idx];
    Some((body, idx + marker.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_template_decl() {
        let src = "[template public genClass(a : EClass)]\nHello\n[/template]";
        let file = parse_templates(src).expect("parse ok");
        assert_eq!(file.declarations.len(), 1);
        let d = &file.declarations[0];
        assert_eq!(d.kind, DeclKind::Template);
        assert_eq!(d.name, "genClass");
        assert!(d.is_public());
        assert_eq!(d.params.len(), 1);
        assert_eq!(d.params[0].name, "a");
        assert_eq!(d.params[0].ty, "EClass");
        assert_eq!(d.body, "\nHello\n");
    }

    #[test]
    fn parses_query_decl() {
        let src = "[query public name(e : EObject) : String = 'x'/]";
        let file = parse_templates(src).expect("parse ok");
        let d = &file.declarations[0];
        assert_eq!(d.kind, DeclKind::Query);
        assert_eq!(d.name, "name");
        assert_eq!(d.params.len(), 1);
    }

    #[test]
    fn skips_module_and_import() {
        let src = "[module bms('/example')/]\n[import other::file /]\n[comment hi /]\n[template private done()]\nx\n[/template]";
        let file = parse_templates(src).expect("parse ok");
        assert_eq!(file.declarations.len(), 1);
        assert_eq!(file.declarations[0].visibility, Visibility::Private);
    }

    #[test]
    fn entry_points_are_public_noarg() {
        let src = "[template public a()]A[/template]\n[template protected b()]B[/template]\n[template public c(x:Int)]C[/template]";
        let file = parse_templates(src).expect("parse ok");
        assert_eq!(file.entry_points().len(), 1);
        assert_eq!(file.entry_points()[0].name, "a");
    }
}
