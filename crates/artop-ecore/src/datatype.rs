//! Ecore built-in data-type helpers (parse from string / convert to string).
//!
//! Port of C++ `emf-ecore/DataTypeUtil` subset that the `EFactory`
//! `createFromString` / `convertToString` signatures rely on. Values are
//! [crate::Val], keyed by the EMF data type name (`EString`, `EInt`, ...).

use crate::Val;

/// Names of the Ecore built-in data types.
pub mod names {
    pub const E_STRING: &str = "EString";
    pub const E_INT: &str = "EInt";
    pub const E_INTEGER_OBJECT: &str = "EIntegerObject";
    pub const E_LONG: &str = "ELong";
    pub const E_LONG_OBJECT: &str = "ELongObject";
    pub const E_DOUBLE: &str = "EDouble";
    pub const E_DOUBLE_OBJECT: &str = "EDoubleObject";
    pub const E_FLOAT: &str = "EFloat";
    pub const E_FLOAT_OBJECT: &str = "EFloatObject";
    pub const E_BOOLEAN: &str = "EBoolean";
    pub const E_BOOLEAN_OBJECT: &str = "EBooleanObject";
    pub const E_BYTE: &str = "EByte";
    pub const E_BYTE_OBJECT: &str = "EByteObject";
    pub const E_SHORT: &str = "EShort";
    pub const E_SHORT_OBJECT: &str = "EShortObject";
    pub const E_CHAR: &str = "EChar";
    pub const E_CHARACTER_OBJECT: &str = "ECharacterObject";
    pub const E_BIG_INTEGER: &str = "EBigInteger";
    pub const E_BIG_DECIMAL: &str = "EBigDecimal";
}

/// Parse a literal string into a [`Val`] for the named Ecore data type.
/// Returns `Val::String(literal)` verbatim for unknown types (lenient).
pub fn from_string(data_type: &str, literal: &str) -> Val {
    match data_type {
        names::E_STRING => Val::String(literal.to_string()),
        names::E_INT | names::E_INTEGER_OBJECT | names::E_LONG | names::E_LONG_OBJECT => literal
            .trim()
            .parse::<i64>()
            .map(Val::Int)
            .unwrap_or(Val::Int(0)),
        names::E_DOUBLE | names::E_DOUBLE_OBJECT | names::E_FLOAT | names::E_FLOAT_OBJECT => {
            literal
                .trim()
                .parse::<f64>()
                .map(Val::Double)
                .unwrap_or(Val::Double(0.0))
        }
        names::E_BOOLEAN | names::E_BOOLEAN_OBJECT => {
            let t = literal.trim();
            Val::Bool(match t {
                "true" | "1" => true,
                _ => false,
            })
        }
        names::E_BYTE | names::E_BYTE_OBJECT => {
            Val::Byte(literal.trim().parse::<u8>().unwrap_or(0))
        }
        names::E_SHORT | names::E_SHORT_OBJECT | names::E_CHAR | names::E_CHARACTER_OBJECT => {
            Val::Int(literal.trim().parse::<i32>().map(|v| v as i64).unwrap_or(0))
        }
        _ => Val::String(literal.to_string()),
    }
}

/// Convert a [`Val`] back to its EMF string representation for the named type.
/// Unknown types fall back to `describe`.
pub fn to_string(data_type: &str, value: &Val) -> String {
    match (data_type, value) {
        (names::E_STRING, Val::String(s)) => s.clone(),
        (_, Val::Int(i)) => i.to_string(),
        (_, Val::Double(d)) => d.to_string(),
        (_, Val::Bool(b)) => b.to_string(),
        (_, Val::Byte(b)) => b.to_string(),
        (_, Val::EnumLiteral(e)) => e.clone(),
        (_, Val::Null) => "".to_string(),
        (names::E_BOOLEAN | names::E_BOOLEAN_OBJECT, Val::String(s)) => s.clone(),
        _ => value.describe(),
    }
}

/// The default value for a named Ecore data type.
pub fn default_value(data_type: &str) -> Val {
    match data_type {
        names::E_STRING => Val::String(String::new()),
        names::E_INT
        | names::E_INTEGER_OBJECT
        | names::E_LONG
        | names::E_LONG_OBJECT
        | names::E_SHORT
        | names::E_SHORT_OBJECT
        | names::E_CHAR
        | names::E_CHARACTER_OBJECT => Val::Int(0),
        names::E_DOUBLE | names::E_DOUBLE_OBJECT | names::E_FLOAT | names::E_FLOAT_OBJECT => {
            Val::Double(0.0)
        }
        names::E_BOOLEAN | names::E_BOOLEAN_OBJECT => Val::Bool(false),
        names::E_BYTE | names::E_BYTE_OBJECT => Val::Byte(0),
        _ => Val::Null,
    }
}
