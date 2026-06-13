use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Bool(b) => write!(f, "{}", b),
        }
    }
}

impl Value {
    /// Parse a raw string with an optional type tag.
    /// E.g. `[INT] 42`, `[STRING] hello`, `"hello"` (auto), `4.2` (auto).
    pub fn parse(raw: &str, explicit_type: Option<&str>) -> Result<Self, String> {
        let raw_trimmed = raw.trim();
        if let Some(t) = explicit_type {
            match t.to_uppercase().as_str() {
                "STRING" => {
                    // If it has quotes, strip them, otherwise keep raw
                    let clean = if raw_trimmed.starts_with('"') && raw_trimmed.ends_with('"') && raw_trimmed.len() >= 2 {
                        &raw_trimmed[1..raw_trimmed.len() - 1]
                    } else {
                        raw_trimmed
                    };
                    Ok(Value::String(clean.to_string()))
                }
                "INT" => {
                    let val = raw_trimmed
                        .parse::<i64>()
                        .map_err(|_| format!("Cannot parse '{}' as INT", raw_trimmed))?;
                    Ok(Value::Int(val))
                }
                "FLOAT" => {
                    let val = raw_trimmed
                        .parse::<f64>()
                        .map_err(|_| format!("Cannot parse '{}' as FLOAT", raw_trimmed))?;
                    Ok(Value::Float(val))
                }
                "BOOL" => {
                    let val = match raw_trimmed.to_lowercase().as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return Err(format!("Cannot parse '{}' as BOOL", raw_trimmed)),
                    };
                    Ok(Value::Bool(val))
                }
                other => Err(format!("Unknown explicit type: {}", other)),
            }
        } else {
            // Automatic detection
            if raw_trimmed.starts_with('"') && raw_trimmed.ends_with('"') && raw_trimmed.len() >= 2 {
                let s = &raw_trimmed[1..raw_trimmed.len() - 1];
                Ok(Value::String(s.to_string()))
            } else if raw_trimmed.to_lowercase() == "true" {
                Ok(Value::Bool(true))
            } else if raw_trimmed.to_lowercase() == "false" {
                Ok(Value::Bool(false))
            } else if let Ok(i) = raw_trimmed.parse::<i64>() {
                Ok(Value::Int(i))
            } else if let Ok(f) = raw_trimmed.parse::<f64>() {
                Ok(Value::Float(f))
            } else {
                // Default fallback is String if it's a word without quotes
                Ok(Value::String(raw_trimmed.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_parsing() {
        // Auto detection
        assert_eq!(Value::parse("\"hello\"", None), Ok(Value::String("hello".to_string())));
        assert_eq!(Value::parse("true", None), Ok(Value::Bool(true)));
        assert_eq!(Value::parse("123", None), Ok(Value::Int(123)));
        assert_eq!(Value::parse("4.56", None), Ok(Value::Float(4.56)));
        assert_eq!(Value::parse("random", None), Ok(Value::String("random".to_string())));

        // Explicit types
        assert_eq!(Value::parse("123", Some("STRING")), Ok(Value::String("123".to_string())));
        assert_eq!(Value::parse("123", Some("INT")), Ok(Value::Int(123)));
        assert_eq!(Value::parse("4.56", Some("FLOAT")), Ok(Value::Float(4.56)));
        assert_eq!(Value::parse("true", Some("BOOL")), Ok(Value::Bool(true)));
    }
}
