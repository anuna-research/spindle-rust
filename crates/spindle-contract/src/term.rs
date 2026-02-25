//! Typed term DTO for v2 JSON schema (REQ-012, CON-006).
//!
//! Provides [`TermDto`] — a JSON-serializable representation of [`spindle_core::term::Term`]
//! that preserves type information:
//!
//! - `Symbol` → `{"type": "symbol", "value": "name"}`
//! - `Integer` → `{"type": "integer", "value": 42}` (string if |n| > 2^53)
//! - `Decimal` → `{"type": "decimal", "value": "8.00"}` (always string, preserves scale)
//! - `Float` → `{"type": "float", "value": 3.14}`

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use spindle_core::intern::resolve;
use spindle_core::term::{FiniteFloat, Term};

/// Maximum safe integer for JSON number representation (2^53).
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_992;

/// A typed term value for v2 JSON output.
///
/// Unlike v1 (which serializes all args as strings), v2 preserves the
/// original type so consumers can distinguish symbols from numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum TermDto {
    /// A symbolic name (e.g., `"widget"`, `"alice"`).
    Symbol(String),
    /// A 64-bit signed integer.
    Integer(i64),
    /// A decimal value as string (preserves trailing zeros / scale).
    Decimal(String),
    /// An IEEE 754 float.
    Float(f64),
}

impl From<&Term> for TermDto {
    fn from(term: &Term) -> Self {
        match term {
            Term::Symbol(id) => TermDto::Symbol(resolve(*id).to_string()),
            Term::Integer(n) => TermDto::Integer(*n),
            Term::Decimal(d) => TermDto::Decimal(d.to_string()),
            Term::Float(f) => TermDto::Float(f.value()),
        }
    }
}

impl TermDto {
    /// Convert back to a [`Term`], returning `None` only if the decimal string
    /// cannot be parsed (should not happen for well-formed DTOs).
    pub fn to_term(&self) -> Option<Term> {
        match self {
            TermDto::Symbol(s) => Some(Term::Symbol(spindle_core::intern::intern(s))),
            TermDto::Integer(n) => Some(Term::Integer(*n)),
            TermDto::Decimal(s) => {
                use std::str::FromStr;
                rust_decimal::Decimal::from_str(s).ok().map(Term::Decimal)
            }
            TermDto::Float(f) => FiniteFloat::new(*f).map(Term::Float),
        }
    }
}

// ---------------------------------------------------------------------------
// Custom Serialize
// ---------------------------------------------------------------------------

impl Serialize for TermDto {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            TermDto::Symbol(s) => {
                map.serialize_entry("type", "symbol")?;
                map.serialize_entry("value", s)?;
            }
            TermDto::Integer(n) => {
                map.serialize_entry("type", "integer")?;
                if n.abs() > MAX_SAFE_INTEGER {
                    // Serialize as string to avoid JSON number precision loss
                    map.serialize_entry("value", &n.to_string())?;
                } else {
                    map.serialize_entry("value", n)?;
                }
            }
            TermDto::Decimal(s) => {
                map.serialize_entry("type", "decimal")?;
                map.serialize_entry("value", s)?;
            }
            TermDto::Float(f) => {
                map.serialize_entry("type", "float")?;
                map.serialize_entry("value", f)?;
            }
        }
        map.end()
    }
}

// ---------------------------------------------------------------------------
// Custom Deserialize
// ---------------------------------------------------------------------------

impl<'de> Deserialize<'de> for TermDto {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(TermDtoVisitor)
    }
}

struct TermDtoVisitor;

impl<'de> Visitor<'de> for TermDtoVisitor {
    type Value = TermDto;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a TermDto object with 'type' and 'value' fields")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut type_str: Option<String> = None;
        let mut value: Option<serde_json::Value> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "type" => type_str = Some(map.next_value()?),
                "value" => value = Some(map.next_value()?),
                _ => {
                    let _: serde_json::Value = map.next_value()?;
                }
            }
        }

        let type_str = type_str.ok_or_else(|| de::Error::missing_field("type"))?;
        let value = value.ok_or_else(|| de::Error::missing_field("value"))?;

        match type_str.as_str() {
            "symbol" => {
                let s = value
                    .as_str()
                    .ok_or_else(|| de::Error::custom("symbol value must be a string"))?;
                Ok(TermDto::Symbol(s.to_string()))
            }
            "integer" => {
                // Accept both number and string (for large integers)
                if let Some(n) = value.as_i64() {
                    Ok(TermDto::Integer(n))
                } else if let Some(s) = value.as_str() {
                    s.parse::<i64>()
                        .map(TermDto::Integer)
                        .map_err(|_| de::Error::custom("invalid integer string"))
                } else {
                    Err(de::Error::custom("integer value must be number or string"))
                }
            }
            "decimal" => {
                let s = value
                    .as_str()
                    .ok_or_else(|| de::Error::custom("decimal value must be a string"))?;
                Ok(TermDto::Decimal(s.to_string()))
            }
            "float" => {
                let f = value
                    .as_f64()
                    .ok_or_else(|| de::Error::custom("float value must be a number"))?;
                Ok(TermDto::Float(f))
            }
            other => Err(de::Error::custom(format!("unknown term type: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_round_trip() {
        let dto = TermDto::Symbol("widget".to_string());
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json, r#"{"type":"symbol","value":"widget"}"#);
        let back: TermDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn integer_small_round_trip() {
        let dto = TermDto::Integer(42);
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json, r#"{"type":"integer","value":42}"#);
        let back: TermDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn integer_large_uses_string() {
        let big = i64::MAX; // 9223372036854775807 > 2^53
        let dto = TermDto::Integer(big);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(
            json.contains(r#""value":"9223372036854775807""#),
            "large integer should serialize as string, got: {json}"
        );
        let back: TermDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn decimal_preserves_scale() {
        let dto = TermDto::Decimal("8.00".to_string());
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json, r#"{"type":"decimal","value":"8.00"}"#);
        let back: TermDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn float_round_trip() {
        let dto = TermDto::Float(3.14);
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json, r#"{"type":"float","value":3.14}"#);
        let back: TermDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn to_term_round_trip() {
        let terms = [
            TermDto::Symbol("hello".to_string()),
            TermDto::Integer(99),
            TermDto::Decimal("1.50".to_string()),
            TermDto::Float(2.5),
        ];
        for dto in &terms {
            let term = dto.to_term().expect("to_term should succeed");
            let back = TermDto::from(&term);
            assert_eq!(dto, &back, "round-trip failed for {:?}", dto);
        }
    }
}
