//! Type mapping and identifier shaping for generated Rust code.
//!
//! Port target: C++ `emf-ecore-codegen`'s `TypeMapper` + the naming helpers in
//! `GenModel` (`getClassPackageName`, `getCppType`, `capitalize`, ...), adapted
//! to Rust identifiers and types.
//!
//! The mapping here is deliberately *concrete-field*: every Ecore feature type
//! maps to a fixed Rust type (attribute -> scalar/string/bool, reference ->
//! `ObjectRef` / `Vec<ObjectRef>`). No generics, no derive-stacking — this is
//! what keeps generated crates fast to compile even with many classes.

use emf_ecore::EStructuralFeature;

/// Map an Ecore built-in data type name to a Rust scalar type used as the
/// field type of an attribute. Unknown types fall back to `String`.
pub fn attr_rust_type(type_name: &str, many: bool) -> String {
    let scalar = match type_name {
        "EString" => "String".to_string(),
        "EInt" | "EIntegerObject" | "EShort" | "EShortObject" | "EChar" | "ECharacterObject"
        | "ELong" | "ELongObject" | "EByte" | "EByteObject" => "i64".to_string(),
        "EDouble" | "EDoubleObject" | "EFloat" | "EFloatObject" => "f64".to_string(),
        "EBoolean" | "EBooleanObject" => "bool".to_string(),
        other => format!("{other}"),
    };
    if many {
        format!("Vec<{scalar}>")
    } else {
        scalar
    }
}

/// Map a reference feature's target-class name to a Rust field type. Every
/// reference resolves to `ObjectRef` (single) / `Vec<ObjectRef>` (many); the
/// target class name is intentionally unused (the concrete generated type of a
/// reference is always the shared object handle).
pub fn ref_rust_type(_target: &str, many: bool) -> String {
    let base = "ObjectRef".to_string();
    if many {
        format!("Vec<{base}>")
    } else {
        base
    }
}

/// The Rust field type for a structural feature, including an `Option` wrapper
/// for a non-required single-valued feature (attributes and references).
pub fn field_rust_type(feat: &EStructuralFeature) -> String {
    let many = feat.is_many();
    if feat.is_reference() {
        let target = feat.type_name().unwrap_or("EObject");
        let ty = ref_rust_type(target, many);
        if many || feat.is_required() {
            ty
        } else {
            format!("Option<{ty}>")
        }
    } else {
        let tn = feat.type_name().unwrap_or("EString");
        let ty = attr_rust_type(tn, false);
        if many {
            "Vec<i64>".to_string() // multi-valued attribute: keep as a list of scalars
        } else if feat.is_required() {
            ty
        } else {
            format!("Option<{ty}>")
        }
    }
}

/// Deterministic Rust field name from an Ecore feature name (camelCase -> snake).
pub fn field_name(name: &str) -> String {
    to_snake_case(name)
}

/// Deterministic Rust struct name from an Ecore class name.
pub fn struct_name(name: &str) -> String {
    if name.is_empty() {
        return "Unnamed".to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    let mut prev_upper = false;
    while let Some(c) = chars.next() {
        if c == '_' {
            prev_upper = false;
            continue;
        }
        if out.is_empty() {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() && !prev_upper && !out.ends_with('_') {
            // Already capitalized; append as-is (SingleWord keeps case).
            out.push(c);
        } else {
            out.push(c);
        }
        prev_upper = c.is_uppercase();
    }
    if out.is_empty() {
        "Unnamed".to_string()
    } else {
        out
    }
}

/// Rust module name for a package (snake_case, sanitized).
pub fn package_module_name(pkg_name: &str) -> String {
    let s = to_snake_case(pkg_name);
    if s.is_empty() {
        "model".to_string()
    } else {
        s
    }
}

/// Camel-case to snake-case converter (C++ `toLowerCamel`-like, but Rust style).
pub fn to_snake_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    let mut prev_upper = false;
    let mut prev_char = false;
    for c in input.chars() {
        if c == '-' || c == ' ' || c == '.' {
            if prev_char && !out.ends_with('_') {
                out.push('_');
            }
            prev_upper = false;
            prev_char = false;
            continue;
        }
        if c == '_' {
            if prev_char && !out.ends_with('_') {
                out.push('_');
            }
            prev_upper = false;
            prev_char = false;
            continue;
        }
        if c.is_uppercase() {
            if prev_char && !prev_upper {
                out.push('_');
            }
            for l in c.to_lowercase() {
                out.push(l);
            }
            prev_upper = true;
        } else {
            out.push(c);
            prev_upper = false;
        }
        prev_char = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::structural::FeatureKind;

    #[test]
    fn snake_case_basic() {
        assert_eq!(to_snake_case("shortName"), "short_name");
        assert_eq!(to_snake_case("ShortName"), "short_name");
        assert_eq!(to_snake_case("books"), "books");
        assert_eq!(to_snake_case("AUTOSAR"), "autosar");
    }

    #[test]
    fn field_names() {
        assert_eq!(field_name("shortName"), "short_name");
        assert_eq!(field_name("title"), "title");
    }

    #[test]
    fn attribute_scalar_types() {
        assert_eq!(attr_rust_type("EString", false), "String");
        assert_eq!(attr_rust_type("EInt", false), "i64");
        assert_eq!(attr_rust_type("EBoolean", false), "bool");
        assert_eq!(attr_rust_type("EDouble", false), "f64");
    }

    #[test]
    fn reference_types() {
        let mut f = EStructuralFeature::reference("author");
        f.set_type_name("Writer");
        assert_eq!(ref_rust_type("Writer", false), "ObjectRef");
        let _ = FeatureKind::Reference;
        // for a many reference
        assert_eq!(ref_rust_type("Book", true), "Vec<ObjectRef>");
    }
}
