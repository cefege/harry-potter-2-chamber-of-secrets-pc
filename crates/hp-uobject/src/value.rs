//! Typed script values and the property-type metadata they decode against.
//!
//! Values are decoded on demand from verbatim payloads using each property's
//! template ([`PropertyKind`]). Struct payloads are binary-serialized in
//! field order (`UStruct::SerializeBin` semantics), arrays are a compact
//! count followed by per-element inner serialization, and map values never
//! serialize in this engine tree.

use crate::error::{Fail, Result};
use hp_format::package79::{ByteCursor, read_compact_index, read_fstring};

use crate::arena::ObjectId;

/// Property template kinds, derived from the serialized `UProperty`
/// subclass of each field export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyKind {
    /// Enum-constrained byte; carries the enum object reference.
    Byte {
        enum_ref: Option<ObjectId>,
    },
    Int,
    Bool,
    Float,
    Object {
        class_ref: Option<ObjectId>,
    },
    Class {
        class_ref: Option<ObjectId>,
        meta_ref: Option<ObjectId>,
    },
    Name,
    Str,
    Struct {
        struct_ref: Option<ObjectId>,
    },
    Array {
        inner: Box<PropertyKind>,
    },
    FixedArray {
        inner: Box<PropertyKind>,
        count: i32,
    },
    Map {
        key: Box<PropertyKind>,
        value: Box<PropertyKind>,
    },
}

/// A runtime script value.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    Byte(u8),
    Int(i32),
    Bool(bool),
    Float(f32),
    /// Raw package reference as stored in the payload; converted to an
    /// [`ObjectId`](crate::arena::ObjectId) by the loader after decoding.
    Object(Option<i32>),
    Name(crate::name::Name),
    Str(String),
    /// Named struct with its own ordered bag.
    Struct {
        struct_name: u32,
        fields: Vec<(u32, PropValue)>,
    },
    Array(Vec<PropValue>),
    FixedArray(Vec<PropValue>),
    Map(Vec<(PropValue, PropValue)>),
}

impl PropValue {
    pub fn default_for(kind: &PropertyKind) -> PropValue {
        match kind {
            PropertyKind::Byte { .. } => PropValue::Byte(0),
            PropertyKind::Int => PropValue::Int(0),
            PropertyKind::Bool => PropValue::Bool(false),
            PropertyKind::Float => PropValue::Float(0.0),
            PropertyKind::Object { .. } | PropertyKind::Class { .. } => PropValue::Object(None),
            PropertyKind::Name => PropValue::Name(crate::name::Name::none()),
            PropertyKind::Str => PropValue::Str(String::new()),
            PropertyKind::Struct { .. } => PropValue::Struct {
                struct_name: 0,
                fields: Vec::new(),
            },
            PropertyKind::Array { .. } => PropValue::Array(Vec::new()),
            PropertyKind::FixedArray { count, .. } => {
                PropValue::FixedArray(vec![PropValue::Byte(0); (*count).max(0) as usize])
            }
            PropertyKind::Map { .. } => PropValue::Map(Vec::new()),
        }
    }

    /// Truthiness per script conversion rules (`!= 0`).
    pub fn truthy(&self) -> bool {
        match self {
            PropValue::Byte(b) => *b != 0,
            PropValue::Int(i) => *i != 0,
            PropValue::Bool(b) => *b,
            PropValue::Float(f) => *f != 0.0,
            PropValue::Object(o) => o.is_some(),
            PropValue::Name(n) => !n.is_none(),
            PropValue::Str(s) => !s.is_empty() && s != "0" && !s.eq_ignore_ascii_case("false"),
            _ => false,
        }
    }
    /// Deterministic byte encoding used by the world snapshot hash.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        use PropValue::*;
        let mut out = Vec::new();
        match self {
            Byte(b) => {
                out.push(1);
                out.push(*b);
            }
            Int(i) => {
                out.push(2);
                out.extend_from_slice(&i.to_le_bytes());
            }
            Bool(b) => {
                out.push(3);
                out.push(u8::from(*b));
            }
            Float(f) => {
                out.push(4);
                out.extend_from_slice(&f.to_le_bytes());
            }
            Object(o) => {
                out.push(5);
                out.extend_from_slice(&o.unwrap_or(-1).to_le_bytes());
            }
            Name(n) => {
                out.push(6);
                out.extend_from_slice(&n.index.to_le_bytes());
                out.extend_from_slice(&n.number.to_le_bytes());
            }
            Str(s) => {
                out.push(7);
                out.extend_from_slice(&(s.len() as u64).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            }
            Struct {
                struct_name,
                fields,
            } => {
                out.push(8);
                out.extend_from_slice(&struct_name.to_le_bytes());
                for (name, value) in fields {
                    out.extend_from_slice(&name.to_le_bytes());
                    out.extend(value.canonical_bytes());
                }
            }
            Array(items) | FixedArray(items) => {
                out.push(9);
                out.extend_from_slice(&(items.len() as u64).to_le_bytes());
                for item in items {
                    out.extend(item.canonical_bytes());
                }
            }
            Map(pairs) => {
                out.push(10);
                out.extend_from_slice(&(pairs.len() as u64).to_le_bytes());
                for (k, v) in pairs {
                    out.extend(k.canonical_bytes());
                    out.extend(v.canonical_bytes());
                }
            }
        }
        out
    }
}

/// Read one package object reference (compact signed index).
pub fn read_object_ref(cur: &mut ByteCursor<'_>) -> Result<i32> {
    Ok(read_compact_index(cur)?)
}

/// Resolves a struct template reference to `(struct name, fields)`.
/// Resolved struct layout: `(struct name, [(field name, kind)])`.
pub type StructFieldList = Option<(u32, Vec<(u32, PropertyKind)>)>;
/// Resolves a struct template reference to its field list.
pub type StructResolver<'a> = &'a dyn Fn(ObjectId) -> Result<StructFieldList>;

fn fixed_dim(kind: &PropertyKind) -> usize {
    match kind {
        PropertyKind::FixedArray { count, .. } => (*count).max(0) as usize,
        _ => 1,
    }
}

/// Decode a value of `kind` from `bytes`.
///
/// `resolve_struct` maps a struct template reference to its field list
/// `(struct name index, [(field name index, kind)])`.
pub fn decode_value(
    cur: &mut ByteCursor<'_>,
    kind: &PropertyKind,
    resolve_struct: StructResolver<'_>,
) -> Result<PropValue> {
    Ok(match kind {
        PropertyKind::Byte { .. } => PropValue::Byte(cur.u8()?),
        PropertyKind::Bool => PropValue::Bool(cur.u8()? != 0),
        PropertyKind::Int => PropValue::Int(cur.i32()?),
        PropertyKind::Float => {
            PropValue::Float(f32::from_le_bytes(cur.take(4)?.try_into().unwrap()))
        }
        PropertyKind::Object { .. } | PropertyKind::Class { .. } => {
            PropValue::Object(Some(read_object_ref(cur)?))
        }
        PropertyKind::Name => PropValue::Name(crate::name::Name {
            index: read_compact_index(cur)? as u32,
            number: crate::name::NO_NUMBER,
        }),
        PropertyKind::Str => PropValue::Str(read_fstring(cur)?),
        PropertyKind::Struct { struct_ref } => {
            let Some(struct_id) = *struct_ref else {
                return Err(Fail::new(
                    "bind.struct_template_missing",
                    "struct property has no resolved template reference",
                ));
            };
            let Some((struct_name, fields)) = resolve_struct(struct_id)? else {
                return Err(Fail::new(
                    "bind.struct_template_missing",
                    format!("struct template {struct_id:?} is not a ScriptStruct"),
                ));
            };
            let mut out = Vec::with_capacity(fields.len());
            for (name_index, field_kind) in &fields {
                for _ in 0..fixed_dim(field_kind) {
                    out.push((*name_index, decode_value(cur, field_kind, resolve_struct)?));
                }
            }
            PropValue::Struct {
                struct_name,
                fields: out,
            }
        }
        PropertyKind::Array { inner } => {
            let count = read_compact_index(cur)?;
            if count < 0 {
                return Err(Fail::new(
                    "bind.negative_array_count",
                    format!("array count {count}"),
                ));
            }
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(decode_value(cur, inner, resolve_struct)?);
            }
            PropValue::Array(items)
        }
        PropertyKind::FixedArray { inner, count } => {
            let mut items = Vec::with_capacity((*count).max(0) as usize);
            for _ in 0..(*count).max(0) {
                items.push(decode_value(cur, inner, resolve_struct)?);
            }
            PropValue::FixedArray(items)
        }
        PropertyKind::Map { .. } => PropValue::Map(Vec::new()),
    })
}
