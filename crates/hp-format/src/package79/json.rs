//! Minimal canonical JSON emitter whose output is byte-identical to
//! `json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True)` from
//! `Build/package79_reference.py::canonical_json` for the audit document's
//! value domain (null / bool / int / string / array / object).

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// JSON value restricted to the shapes the audit document needs.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn str(text: impl Into<String>) -> Json {
        Json::Str(text.into())
    }

    /// Build an object from key/value pairs (keys sorted on serialization).
    pub fn obj(pairs: Vec<(&str, Json)>) -> Json {
        Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// Serialize with two-space indentation plus the trailing newline,
    /// mirroring `canonical_json`.
    pub fn to_canonical(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, indent: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(value) => {
                let _ = write!(out, "{value}");
            }
            Json::Str(text) => escape_into(text, out),
            Json::Arr(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (index, item) in items.iter().enumerate() {
                    push_indent(out, indent + 1);
                    item.write(out, indent + 1);
                    if index + 1 != items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                push_indent(out, indent);
                out.push(']');
            }
            Json::Obj(map) => {
                if map.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                let last = map.len() - 1;
                for (index, (key, value)) in map.iter().enumerate() {
                    push_indent(out, indent + 1);
                    escape_into(key, out);
                    out.push_str(": ");
                    value.write(out, indent + 1);
                    if index != last {
                        out.push(',');
                    }
                    out.push('\n');
                }
                push_indent(out, indent);
                out.push('}');
            }
        }
    }
}

fn push_indent(out: &mut String, levels: usize) {
    for _ in 0..levels {
        out.push_str("  ");
    }
}

fn escape_into(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
