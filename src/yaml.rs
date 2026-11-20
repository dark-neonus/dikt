//! Strict YAML loading into JSON values and a deterministic block-style emitter.
//!
//! Loading rejects duplicate keys, non-string keys and custom tags. YAML 1.2
//! core resolution keeps dates as strings (no implicit timestamps).
//! Emitting uses plain scalars only when they unambiguously round-trip as
//! strings; everything else is double-quoted with JSON escaping.

use serde_json::{Map, Number, Value};
use serde_norway::Value as Yaml;

use crate::bail;
use crate::error::{Error, Result};

pub fn load(text: &str) -> Result<Value> {
    let document: Yaml = serde_norway::from_str(text).map_err(|error| {
        let message = error.to_string();
        if message.contains("duplicate entry") {
            Error::new("Duplicate YAML mapping keys")
        } else {
            Error(format!("Invalid YAML: {message}"))
        }
    })?;
    convert(document)
}

fn convert(value: Yaml) -> Result<Value> {
    Ok(match value {
        Yaml::Null => Value::Null,
        Yaml::Bool(flag) => Value::Bool(flag),
        Yaml::Number(number) => {
            if let Some(integer) = number.as_i64() {
                Value::from(integer)
            } else if let Some(integer) = number.as_u64() {
                Value::from(integer)
            } else {
                match number.as_f64().and_then(Number::from_f64) {
                    Some(float) => Value::Number(float),
                    None => bail!("Invalid YAML: non-finite numbers are not supported"),
                }
            }
        }
        Yaml::String(text) => Value::String(text),
        Yaml::Sequence(entries) => {
            Value::Array(entries.into_iter().map(convert).collect::<Result<_>>()?)
        }
        Yaml::Mapping(mapping) => {
            let mut result = Map::new();
            for (key, entry) in mapping {
                let Yaml::String(key) = key else {
                    bail!("YAML mapping keys must be strings");
                };
                if result.insert(key, convert(entry)?).is_some() {
                    bail!("Duplicate YAML mapping keys");
                }
            }
            Value::Object(result)
        }
        Yaml::Tagged(_) => bail!("Invalid YAML: custom tags are not supported"),
    })
}

const RESERVED: [&str; 9] = ["null", "true", "false", "yes", "no", "on", "off", "y", "n"];

fn plain(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " _.,/()'+-".contains(c))
        && !text.ends_with(' ')
        && !RESERVED.contains(&text.to_ascii_lowercase().as_str())
}

pub fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) if plain(text) => text.clone(),
        Value::String(text) => serde_json::to_string(text).expect("string serialization"),
        Value::Array(entries) if entries.is_empty() => "[]".into(),
        Value::Object(map) if map.is_empty() => "{}".into(),
        _ => unreachable!("collections are emitted as blocks"),
    }
}

fn is_block(value: &Value) -> bool {
    matches!(value, Value::Array(a) if !a.is_empty())
        || matches!(value, Value::Object(m) if !m.is_empty())
}

fn mapping(out: &mut String, map: &Map<String, Value>, indent: usize) {
    let pad = " ".repeat(indent);
    for (key, value) in map {
        let key = scalar(&Value::String(key.clone()));
        if is_block(value) {
            out.push_str(&format!("{pad}{key}:\n"));
            match value {
                Value::Object(inner) => mapping(out, inner, indent + 2),
                Value::Array(entries) => sequence(out, entries, indent),
                _ => unreachable!(),
            }
        } else {
            out.push_str(&format!("{pad}{key}: {}\n", scalar(value)));
        }
    }
}

fn sequence(out: &mut String, entries: &[Value], indent: usize) {
    let pad = " ".repeat(indent);
    for entry in entries {
        if is_block(entry) {
            let mut nested = String::new();
            match entry {
                Value::Object(inner) => mapping(&mut nested, inner, indent + 2),
                Value::Array(inner) => sequence(&mut nested, inner, indent + 2),
                _ => unreachable!(),
            }
            out.push_str(&pad);
            out.push_str("- ");
            out.push_str(&nested[indent + 2..]);
        } else {
            out.push_str(&format!("{pad}- {}\n", scalar(entry)));
        }
    }
}

/// Emit a top-level mapping in block style, preserving key order.
pub fn dump(map: &Map<String, Value>) -> String {
    let mut out = String::new();
    mapping(&mut out, map, 0);
    out
}

/// Python `json.dumps(value, ensure_ascii=False)` formatting (", " and ": " separators),
/// used for human-readable log entries.
pub fn py_json(value: &Value) -> String {
    match value {
        Value::Array(entries) => format!(
            "[{}]",
            entries.iter().map(py_json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(key, entry)| format!(
                    "{}: {}",
                    serde_json::to_string(key).unwrap(),
                    py_json(entry)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_preserves_values_and_order() {
        let value = json!({
            "schema_version": 1, "id": "T-00001", "title": "Fix: caf\u{e9} \"quoted\"\nline",
            "tags": ["pm", "yes"], "due": "2026-12-01", "none": null, "empty": [],
            "links": {"jira": [], "items": ["I-00001"]},
            "checklist": [{"text": "Test", "done": false}, {"text": "true", "done": true}],
            "nested": [[1, 2], []], "float": 1.5,
        });
        let text = dump(value.as_object().unwrap());
        assert!(text.contains("schema_version: 1\n"));
        assert!(text.contains("  jira: []\n"));
        assert!(text.contains("- text: Test\n  done: false\n"));
        assert!(text.contains("due: \"2026-12-01\"\n"));
        assert_eq!(load(&text).unwrap(), value);
        let keys: Vec<_> = load(&text)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            value
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn rejects_duplicates_and_non_string_keys() {
        assert_eq!(
            load("a: 1\na: 2\n").unwrap_err().message(),
            "Duplicate YAML mapping keys"
        );
        assert!(
            load("1: a\n")
                .unwrap_err()
                .message()
                .contains("keys must be strings")
        );
        assert!(load("a: !custom x\n").is_err());
    }

    #[test]
    fn dates_stay_strings() {
        assert_eq!(
            load("due: 2026-12-01\n").unwrap(),
            json!({"due": "2026-12-01"})
        );
    }
}
