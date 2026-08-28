//! Pure-native implementations over the evaluated-argument call ABI.
//!
//! Selection is data-driven: [`lookup`] maps a bound function's dotted
//! subject path to an implementation by case-folded leaf name, mirroring
//! how the engine registers `GNatives` bodies by function identity rather
//! than by hard-coded slot arithmetic. Slots whose leaf name is absent
//! here fall back to [`crate::natives::deferred_native`] — loud, counted,
//! never silent.
//!
//! Scope:
//! - implemented: value operators and intrinsics plus lvalue ABI entries for
//!   float `+=` and int pre/post increment/decrement;
//! - remaining compound families stay loudly deferred, but `Frame` recognizes
//!   their verified `P_GET_*_REF` slot metadata so future bodies receive the
//!   same typed lvalue ABI. `UnCorSc.cpp` is the source convention:
//!   prefix returns the assigned value; postfix returns the original value.
//! State-machine natives and latents are also implemented through the
//! embedding scheduler.

use crate::arena::{ObjectData, ObjectId};
use crate::error::{Fail, Result};
use crate::name::{NAME_NONE, Name};
use crate::natives::CallArgs;
use crate::value::PropValue;

/// Human-facing documentation anchor for the RNG contract (kept here so
/// grepping "LCG" lands on the implementation notes above).
pub const FRAME_RNG_DOC: &str = "per-frame LCG x' = x*1664525 + 1013904223, seeded 0x20160611";

type VmResult<T> = std::result::Result<T, Fail>;
type Frame<'a> = crate::vm::Frame<'a>;

/// Resolve a bound subject path (`Engine.Actor.PlaySound`) to its pure
/// implementation, if one exists. Case-folded leaf comparison.
pub fn lookup(subject: &str) -> Option<NativeImpl> {
    let leaf = subject.rsplit('.').next().unwrap_or(subject);
    let folded = crate::name::fold_key(leaf);
    TABLE.iter().find(|(n, _)| *n == folded).map(|(_, f)| *f)
}

use crate::natives::NativeImpl;

/// One argument coerced to float (script numeric promotion rules).
fn f_arg(args: &CallArgs, i: usize) -> f32 {
    match args.get(i) {
        Some(PropValue::Byte(b)) => f32::from(*b),
        Some(PropValue::Int(v)) => *v as f32,
        Some(PropValue::Bool(b)) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Some(PropValue::Float(f)) => *f,
        _ => 0.0,
    }
}
fn f_value(value: &PropValue) -> f32 {
    match value {
        PropValue::Byte(value) => f32::from(*value),
        PropValue::Int(value) => *value as f32,
        PropValue::Bool(value) => f32::from(*value),
        PropValue::Float(value) => *value,
        _ => 0.0,
    }
}

/// One argument coerced to int.
fn i_arg(args: &CallArgs, i: usize) -> i32 {
    match args.get(i) {
        Some(PropValue::Byte(b)) => i32::from(*b),
        Some(PropValue::Int(v)) => *v,
        Some(PropValue::Bool(b)) => i32::from(*b),
        Some(PropValue::Float(f)) => *f as i32,
        _ => 0,
    }
}
fn i_value(value: &PropValue) -> i32 {
    match value {
        PropValue::Byte(value) => i32::from(*value),
        PropValue::Int(value) => *value,
        PropValue::Bool(value) => i32::from(*value),
        PropValue::Float(value) => *value as i32,
        _ => 0,
    }
}

/// One argument coerced to bool (`!= 0` truthiness).
fn b_arg(args: &CallArgs, i: usize) -> bool {
    args.get(i).map(|v| v.truthy()).unwrap_or(false)
}

/// One argument coerced to a display string.
fn s_arg(args: &CallArgs, i: usize) -> String {
    match args.get(i) {
        Some(PropValue::Str(s)) => s.clone(),
        Some(PropValue::Byte(value)) => value.to_string(),
        Some(PropValue::Int(value)) => value.to_string(),
        Some(PropValue::Float(value)) => value.to_string(),
        Some(PropValue::Bool(value)) => if *value { "True" } else { "False" }.to_string(),
        Some(PropValue::Object(None)) => "None".to_string(),
        Some(other) => format!("{other:?}"),
        None => String::new(),
    }
}

/// Object-reference payload of an argument (`None` when null/absent).
fn obj_arg(args: &CallArgs, i: usize) -> Option<i32> {
    match args.get(i) {
        Some(PropValue::Object(o)) => *o,
        _ => None,
    }
}

// ---- Struct templates -------------------------------------------------------

/// `(struct name index, field name indices)` for a Core script struct,
/// matched against the loaded arena (`Core.Vector` → X/Y/Z floats).
fn struct_template(frame: &Frame<'_>, path: &str, fields: &[&str]) -> Option<(u32, Vec<u32>)> {
    let wanted_struct = path.rsplit('.').next()?;
    let sid = frame.arena.find_by_path(path).or_else(|| {
        frame
            .arena
            .objects_iter()
            .find(|(_, object)| {
                matches!(object.data, ObjectData::ScriptStruct(_))
                    && frame
                        .arena
                        .names
                        .text(object.name_index)
                        .is_some_and(|name| name.eq_ignore_ascii_case(wanted_struct))
            })
            .map(|(id, _)| id)
    })?;
    let object = frame.arena.get(sid).ok()?;
    let name_index = object.name_index;
    let children = match &object.data {
        ObjectData::ScriptStruct(d) => d.children.clone(),
        _ => return None,
    };
    let mut out = Vec::with_capacity(fields.len());
    for (i, wanted) in fields.iter().enumerate() {
        let declared = children
            .get(i)
            .and_then(|c| frame.arena.get(*c).ok())
            .map(|o| o.name_index);
        let index = match declared {
            Some(idx) if matches_field(frame, idx, wanted) => idx,
            _ => frame.arena.names.find_index(wanted)?,
        };
        out.push(index);
    }
    Some((name_index, out))
}

fn matches_field(frame: &Frame<'_>, index: u32, wanted: &str) -> bool {
    frame
        .arena
        .names
        .text(index)
        .map(|t| crate::name::fold_key(t) == crate::name::fold_key(wanted))
        .unwrap_or(false)
}

const VECTOR_FIELDS: [&str; 3] = ["X", "Y", "Z"];
const ROTATOR_FIELDS: [&str; 3] = ["Pitch", "Yaw", "Roll"];

/// Build a `Vector` PropValue from components (field names resolved
/// against the loaded `Core.Vector` script struct).
pub fn vector_value(frame: &Frame<'_>, comps: [f32; 3]) -> VmResult<PropValue> {
    let (struct_name, fields) =
        struct_template(frame, "Core.Vector", &VECTOR_FIELDS).ok_or_else(|| {
            Fail::new(
                "vm.struct_missing",
                "Core.Vector script struct not present in the arena",
            )
        })?;
    Ok(PropValue::Struct {
        struct_name,
        fields: fields
            .into_iter()
            .zip(comps)
            .map(|(n, v)| (n, PropValue::Float(v)))
            .collect(),
    })
}

/// Build a `Rotator` PropValue from integer pitch/yaw/roll.
pub fn rotator_value(frame: &Frame<'_>, comps: [i32; 3]) -> VmResult<PropValue> {
    let (struct_name, fields) = struct_template(frame, "Core.Rotator", &ROTATOR_FIELDS)
        .ok_or_else(|| {
            Fail::new(
                "vm.struct_missing",
                "Core.Rotator script struct not present in the arena",
            )
        })?;
    Ok(PropValue::Struct {
        struct_name,
        fields: fields
            .into_iter()
            .zip(comps)
            .map(|(n, v)| (n, PropValue::Int(v)))
            .collect(),
    })
}

/// Read `[x, y, z]` out of a Vector-shaped PropValue argument.
fn vec_of(frame: &Frame<'_>, args: &CallArgs, i: usize) -> Option<[f32; 3]> {
    let PropValue::Struct { fields, .. } = args.get(i)? else {
        return None;
    };
    let mut out = [0.0f32; 3];
    for (name, value) in fields {
        let slot = match frame.arena.names.text(*name)?.to_ascii_uppercase().as_str() {
            "X" => 0,
            "Y" => 1,
            "Z" => 2,
            _ => continue,
        };
        if let PropValue::Float(f) = value {
            out[slot] = *f;
        }
    }
    Some(out)
}

fn vec_result(frame: &Frame<'_>, v: [f32; 3]) -> VmResult<PropValue> {
    vector_value(frame, v)
}

fn rot_result(frame: &Frame<'_>, r: [i32; 3]) -> VmResult<PropValue> {
    rotator_value(frame, r)
}

// ---- Math -------------------------------------------------------------------

fn nat_add_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).wrapping_add(i_arg(a, 1))))
}
fn nat_sub_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).wrapping_sub(i_arg(a, 1))))
}
fn nat_mul_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).wrapping_mul(i_arg(a, 1))))
}
fn nat_div_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let b = i_arg(a, 1);
    Ok(if b == 0 {
        PropValue::Int(0)
    } else {
        PropValue::Int(i_arg(a, 0).wrapping_div(b))
    })
}
fn nat_less_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(i_arg(a, 0) < i_arg(a, 1)))
}
fn nat_greater_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(i_arg(a, 0) > i_arg(a, 1)))
}
fn nat_lessequal_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(i_arg(a, 0) <= i_arg(a, 1)))
}
fn nat_greaterequal_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(i_arg(a, 0) >= i_arg(a, 1)))
}
fn nat_equal_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(i_arg(a, 0) == i_arg(a, 1)))
}
fn nat_notequal_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(i_arg(a, 0) != i_arg(a, 1)))
}
fn nat_and_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0) & i_arg(a, 1)))
}
fn nat_or_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0) | i_arg(a, 1)))
}
fn nat_xor_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0) ^ i_arg(a, 1)))
}
fn nat_complement_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(!i_arg(a, 0)))
}
fn nat_shl_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).wrapping_shl(i_arg(a, 1) as u32)))
}
fn nat_shr_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).wrapping_shr(i_arg(a, 1) as u32)))
}
fn nat_shr_logical_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(
        ((i_arg(a, 0) as u32) >> (i_arg(a, 1) as u32 & 31)) as i32,
    ))
}
fn nat_neg_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).wrapping_neg()))
}
fn nat_min_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).min(i_arg(a, 1))))
}
fn nat_max_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).max(i_arg(a, 1))))
}
fn nat_clamp_int(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(i_arg(a, 0).clamp(i_arg(a, 1), i_arg(a, 2))))
}
fn nat_rand(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let max = i_arg(a, 0);
    Ok(if max <= 0 {
        PropValue::Int(0)
    } else {
        PropValue::Int((f.next_rand_u32() as i32).rem_euclid(max))
    })
}
fn required_lvalue(args: &CallArgs) -> Result<crate::vm::LValue> {
    args.lvalue(0).cloned().ok_or_else(|| {
        Fail::new(
            "vm.native_requires_lvalue",
            "compound/increment native first operand is not a writable lvalue",
        )
    })
}

fn nat_add_equal_int(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let value = PropValue::Int(i_value(&current).wrapping_add(i_arg(a, 1)));
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}

fn nat_preincrement_int(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let value = PropValue::Int(i_value(&current).wrapping_add(1));
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}
fn nat_postincrement_int(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let previous = PropValue::Int(i_value(&current));
    let assigned = PropValue::Int(i_value(&current).wrapping_add(1));
    f.write_lvalue(&target, assigned)?;
    Ok(previous)
}
fn nat_predecrement_int(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let value = PropValue::Int(i_value(&current).wrapping_sub(1));
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}
fn nat_postdecrement_int(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let previous = PropValue::Int(i_value(&current));
    let assigned = PropValue::Int(i_value(&current).wrapping_sub(1));
    f.write_lvalue(&target, assigned)?;
    Ok(previous)
}

fn nat_add_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0) + f_arg(a, 1)))
}
fn nat_sub_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0) - f_arg(a, 1)))
}
fn nat_mul_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0) * f_arg(a, 1)))
}
fn nat_div_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let divisor = f_arg(a, 1);
    Ok(if divisor == 0.0 {
        PropValue::Float(0.0)
    } else {
        PropValue::Float(f_arg(a, 0) / divisor)
    })
}
fn nat_add_equal_float(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    // P_GET_*_REF binds an address before RHS evaluation. Re-read it now:
    // the RHS may itself have mutated the same lvalue.
    let current = f.read_lvalue(&target)?;
    let value = PropValue::Float(f_value(&current) + f_arg(a, 1));
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}
fn nat_multiply_equal_float(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let value = PropValue::Float(f_value(&current) * f_arg(a, 1));
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}

fn nat_pow_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).powf(f_arg(a, 1))))
}
fn nat_mod_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // UE1 `%`: A - B * floor(A/B).
    let (x, y) = (f_arg(a, 0), f_arg(a, 1));
    Ok(PropValue::Float(x - y * (x / y).floor()))
}
fn nat_less_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(f_arg(a, 0) < f_arg(a, 1)))
}
fn nat_greater_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(f_arg(a, 0) > f_arg(a, 1)))
}
fn nat_lessequal_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(f_arg(a, 0) <= f_arg(a, 1)))
}
fn nat_greaterequal_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(f_arg(a, 0) >= f_arg(a, 1)))
}
fn nat_equal_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(f_arg(a, 0) == f_arg(a, 1)))
}
fn nat_notequal_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(f_arg(a, 0) != f_arg(a, 1)))
}
fn nat_complement_equal_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // `~=` approximate float equality; tolerance documented at 1e-4.
    Ok(PropValue::Bool((f_arg(a, 0) - f_arg(a, 1)).abs() < 1e-4))
}
fn nat_neg_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(-f_arg(a, 0)))
}
fn nat_abs_float(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).abs()))
}
fn nat_sin(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).sin()))
}
fn nat_cos(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).cos()))
}
fn nat_tan(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).tan()))
}
fn nat_atan(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).atan()))
}
fn nat_exp(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).exp()))
}
fn nat_loge(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).ln()))
}
fn nat_sqrt(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let v = f_arg(a, 0);
    Ok(PropValue::Float(if v <= 0.0 { 0.0 } else { v.sqrt() }))
}
fn nat_square(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let v = f_arg(a, 0);
    Ok(PropValue::Float(v * v))
}
fn nat_fmin(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).min(f_arg(a, 1))))
}
fn nat_fmax(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(f_arg(a, 0).max(f_arg(a, 1))))
}
fn nat_fclamp(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Float(
        f_arg(a, 0).clamp(f_arg(a, 1), f_arg(a, 2)),
    ))
}
fn nat_lerp(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (a0, b, alpha) = (f_arg(a, 0), f_arg(a, 1), f_arg(a, 2));
    Ok(PropValue::Float(a0 + (b - a0) * alpha.clamp(0.0, 1.0)))
}
fn nat_smerp(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // UE1 smooth interpolation: ((3 - 2*alpha) * alpha^2)(B - A) + A.
    let (x, y, alpha) = (f_arg(a, 0), f_arg(a, 1), f_arg(a, 2).clamp(0.0, 1.0));
    Ok(PropValue::Float(
        ((3.0 - 2.0 * alpha) * alpha * alpha) * (y - x) + x,
    ))
}
fn nat_frand(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    // Top 24 bits mapped into [0,1) — deterministic per frame seed.
    Ok(PropValue::Float(
        (f.next_rand_u32() >> 8) as f32 / (1 << 24) as f32,
    ))
}
fn nat_vrand(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    let angle = (f.next_rand_u32() >> 8) as f32 / (1 << 24) as f32 * std::f32::consts::TAU;
    let z = (f.next_rand_u32() >> 8) as f32 / (1 << 24) as f32 * 2.0 - 1.0;
    let r = (1.0 - z * z).max(0.0).sqrt();
    vec_result(f, [r * angle.cos(), r * angle.sin(), z])
}
fn nat_rotrand(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    let p = f.next_rand_u32() as i32;
    let y = f.next_rand_u32() as i32;
    let r = f.next_rand_u32() as i32;
    rot_result(f, [p, y, r])
}

// ---- Bool / Name / Object ---------------------------------------------------

fn nat_not_bool(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(!b_arg(a, 0)))
}
fn nat_andand_bool(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(b_arg(a, 0) && b_arg(a, 1)))
}
fn nat_oror_bool(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(b_arg(a, 0) || b_arg(a, 1)))
}
fn nat_xorxor_bool(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(b_arg(a, 0) ^ b_arg(a, 1)))
}
fn nat_equal_bool(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(b_arg(a, 0) == b_arg(a, 1)))
}
fn nat_notequal_bool(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(b_arg(a, 0) != b_arg(a, 1)))
}
fn nat_equal_name(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let eq = matches!(
        (args_name(a, 0), args_name(a, 1)),
        (Some(x), Some(y)) if x == y
    );
    Ok(PropValue::Bool(eq))
}
fn nat_notequal_name(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let ne = !matches!(
        (args_name(a, 0), args_name(a, 1)),
        (Some(x), Some(y)) if x == y
    );
    Ok(PropValue::Bool(ne))
}
fn args_name(a: &CallArgs, i: usize) -> Option<crate::name::Name> {
    match a.get(i) {
        Some(PropValue::Name(n)) => Some(*n),
        _ => None,
    }
}
fn nat_equal_object(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(obj_arg(a, 0) == obj_arg(a, 1)))
}
fn nat_notequal_object(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(obj_arg(a, 0) != obj_arg(a, 1)))
}

// ---- Strings ----------------------------------------------------------------

fn nat_concat_str(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let mut out = s_arg(a, 0);
    out.push_str(&s_arg(a, 1));
    Ok(PropValue::Str(out))
}
fn nat_at_str(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let s = s_arg(a, 0);
    let i = i_arg(a, 1);
    Ok(match s.bytes().nth(i.max(0) as usize) {
        Some(b) => PropValue::Str((b as char).to_string()),
        None => PropValue::Str(String::new()),
    })
}
fn nat_len(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(s_arg(a, 0).len() as i32))
}
fn nat_instr(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let hay = s_arg(a, 0);
    let needle = s_arg(a, 1);
    Ok(match hay.find(&needle) {
        Some(pos) => PropValue::Int(pos as i32),
        None => PropValue::Int(-1),
    })
}
fn nat_mid(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // Mid(S, start[, count]) — UE1 indexes strings from zero; omitted
    // count runs to the end.
    let s = s_arg(a, 0);
    let start = (i_arg(a, 1).max(0) as usize).min(s.len());
    let len = a.get(2).map(|_| i_arg(a, 2));
    let end = match len {
        Some(l) => (start + l.max(0) as usize).min(s.len()),
        None => s.len(),
    };
    Ok(PropValue::Str(s[start..end].to_string()))
}
fn nat_left(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let s = s_arg(a, 0);
    let n = (i_arg(a, 1).max(0) as usize).min(s.len());
    Ok(PropValue::Str(s[..n].to_string()))
}
fn nat_right(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let s = s_arg(a, 0);
    let n = (i_arg(a, 1).max(0) as usize).min(s.len());
    Ok(PropValue::Str(s[s.len() - n..].to_string()))
}
fn nat_caps(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Str(s_arg(a, 0).to_ascii_uppercase()))
}
fn nat_chr(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Str(
        char::from_u32(i_arg(a, 0) as u32)
            .unwrap_or('\0')
            .to_string(),
    ))
}
fn nat_asc(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Int(
        s_arg(a, 0).bytes().next().unwrap_or(0) as i32
    ))
}
fn nat_equal_str(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(s_arg(a, 0) == s_arg(a, 1)))
}
fn nat_notequal_str(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(s_arg(a, 0) != s_arg(a, 1)))
}
fn nat_complement_equal_str(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // UE1 string `~=` is case-insensitive equality.
    Ok(PropValue::Bool(
        s_arg(a, 0).eq_ignore_ascii_case(&s_arg(a, 1)),
    ))
}

// ---- Diagnostics / misc -----------------------------------------------------

/// Upper bound on Log/Warn lines printed per process; hot script loops
/// must not flood captured stderr (deterministic: first N calls win).
static LOG_LINES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const MAX_LOG_LINES: usize = 256;

fn log_line(prefix: &str, message: &str) -> bool {
    let n = LOG_LINES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n < MAX_LOG_LINES {
        eprintln!("hp-script{prefix}: {message}");
        true
    } else {
        if n == MAX_LOG_LINES {
            eprintln!("hp-script: log line budget reached; further lines suppressed");
        }
        false
    }
}

fn nat_log(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let message = s_arg(a, 0);
    log_line("", &message);
    f.log_effect(message)?;
    Ok(PropValue::Int(0))
}
fn nat_warn(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    log_line(" warn", &s_arg(a, 0));
    Ok(PropValue::Int(0))
}

/// `Engine.Actor.Error`: intentional actor-local failure. The engine consumes
/// this signal by destroying the receiving actor and discarding its current
/// frame; it must never enter the deferral policy.
fn nat_actor_error(_f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Err(Fail::new("engine.actor_error", s_arg(a, 0)))
}
fn nat_save_config(_f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    // Config persistence is an engine-runtime concern; the script-visible
    // effect this slice is "call succeeds".
    Ok(PropValue::Int(0))
}
fn nat_dynamic_load_object(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let path = s_arg(a, 0);
    if let Some(object) = f.arena.find_by_path(&path) {
        return Ok(PropValue::Object(Some(object.0 as i32)));
    }
    let requested_sound = object_id(f, a, 1, "dynamic_load_object")?
        .and_then(|class| f.arena.path_of(class).ok())
        .is_some_and(|class| {
            class
                .rsplit('.')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("Sound"))
        });
    if requested_sound {
        f.effect(crate::vm::ScriptEffect::AudioUnavailable {
            actor: f.self_id,
            source: format!("DynamicLoadObject {path}"),
        })?;
    }
    Ok(PropValue::Object(None))
}

fn nat_get_sound_duration(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let source = object_id(f, a, 0, "get_sound_duration")?
        .and_then(|sound| f.arena.path_of(sound).ok())
        .unwrap_or_else(|| "None".to_string());
    f.effect(crate::vm::ScriptEffect::AudioUnavailable {
        actor: f.self_id,
        source: format!("GetSoundDuration {source}: duration metadata unavailable"),
    })?;
    // The authored dialog path adds its own 0.5s slop before issuing the cue.
    // Returning zero is the source failure value and still lets that TimedCue
    // complete; inventing a duration would unblock WAITFOR at a false time.
    Ok(PropValue::Float(0.0))
}

/// `IsA(name)` — true when any class on `self`'s chain carries the name.
fn nat_is_a(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let wanted = s_arg(a, 0);
    let class_id = f.self_id.and_then(|id| f.arena.get(id).ok()?.class_id);
    let Some(class_id) = class_id else {
        return Ok(PropValue::Bool(false));
    };
    for class in f.arena.class_chain(class_id)? {
        if let Ok(obj) = f.arena.get(class)
            && let Some(text) = f.arena.names.text(obj.name_index)
            && crate::name::fold_key(text) == crate::name::fold_key(&wanted)
        {
            return Ok(PropValue::Bool(true));
        }
    }
    Ok(PropValue::Bool(false))
}

/// `ClassIsChildOf(cls, base)` over raw object references.
fn nat_class_is_child_of(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let to_id = |raw: Option<i32>| -> Option<ObjectId> {
        raw.filter(|v| *v >= 0).map(|v| ObjectId(v as u32))
    };
    let Some(cls) = to_id(obj_arg(a, 0)) else {
        return Ok(PropValue::Bool(false));
    };
    let Some(base) = to_id(obj_arg(a, 1)) else {
        return Ok(PropValue::Bool(false));
    };
    Ok(PropValue::Bool(f.arena.class_is_a(cls, base)?))
}

// ---- Vectors ----------------------------------------------------------------

fn nat_add_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (u, v) = (
        vec_of(f, a, 0).unwrap_or_default(),
        vec_of(f, a, 1).unwrap_or_default(),
    );
    vec_result(f, [u[0] + v[0], u[1] + v[1], u[2] + v[2]])
}
fn nat_add_equal_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let PropValue::Struct { fields, .. } = &current else {
        return Err(Fail::new(
            "vm.native_argument_type",
            format!("AddEqual_VectorVector target is not a Vector: {current:?}"),
        ));
    };
    let mut lhs = [0.0f32; 3];
    for (name, value) in fields {
        let slot = match f
            .arena
            .names
            .text(*name)
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str()
        {
            "X" => 0,
            "Y" => 1,
            "Z" => 2,
            _ => continue,
        };
        if let PropValue::Float(value) = value {
            lhs[slot] = *value;
        }
    }
    let rhs = vec_of(f, a, 1).unwrap_or_default();
    let value = vec_result(f, [lhs[0] + rhs[0], lhs[1] + rhs[1], lhs[2] + rhs[2]])?;
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}

fn nat_sub_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (u, v) = (
        vec_of(f, a, 0).unwrap_or_default(),
        vec_of(f, a, 1).unwrap_or_default(),
    );
    vec_result(f, [u[0] - v[0], u[1] - v[1], u[2] - v[2]])
}
fn nat_neg_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let v = vec_of(f, a, 0).unwrap_or_default();
    vec_result(f, [-v[0], -v[1], -v[2]])
}
fn nat_mul_vec_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // UE1 componentwise vector product.
    let (u, v) = (
        vec_of(f, a, 0).unwrap_or_default(),
        vec_of(f, a, 1).unwrap_or_default(),
    );
    vec_result(f, [u[0] * v[0], u[1] * v[1], u[2] * v[2]])
}
fn nat_mul_vec_float(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (v, s) = (vec_of(f, a, 0).unwrap_or_default(), f_arg(a, 1));
    vec_result(f, [v[0] * s, v[1] * s, v[2] * s])
}
fn nat_mul_float_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (s, v) = (f_arg(a, 0), vec_of(f, a, 1).unwrap_or_default());
    vec_result(f, [v[0] * s, v[1] * s, v[2] * s])
}
fn nat_div_vec_float(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (v, s) = (vec_of(f, a, 0).unwrap_or_default(), f_arg(a, 1));
    if s == 0.0 {
        return vec_result(f, [0.0; 3]);
    }
    vec_result(f, [v[0] / s, v[1] / s, v[2] / s])
}
fn nat_equal_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let eq = vec_of(f, a, 0) == vec_of(f, a, 1);
    Ok(PropValue::Bool(eq))
}
fn nat_notequal_vec(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let ne = vec_of(f, a, 0) != vec_of(f, a, 1);
    Ok(PropValue::Bool(ne))
}
fn nat_dot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (u, v) = (
        vec_of(f, a, 0).unwrap_or_default(),
        vec_of(f, a, 1).unwrap_or_default(),
    );
    Ok(PropValue::Float(u[0] * v[0] + u[1] * v[1] + u[2] * v[2]))
}

fn nat_cross(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (u, v) = (
        vec_of(f, a, 0).unwrap_or_default(),
        vec_of(f, a, 1).unwrap_or_default(),
    );
    vec_result(
        f,
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ],
    )
}

fn nat_vsize(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let v = vec_of(f, a, 0).unwrap_or_default();
    Ok(PropValue::Float(
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
    ))
}

fn nat_normal(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let v = vec_of(f, a, 0).unwrap_or_default();
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 {
        return vec_result(f, [0.0; 3]);
    }
    vec_result(f, [v[0] / len, v[1] / len, v[2] / len])
}

fn nat_mirror_vec_by_normal(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // UE1: V - (2 * (V dot Z)) * Z.
    let (v, n) = (
        vec_of(f, a, 0).unwrap_or_default(),
        vec_of(f, a, 1).unwrap_or_default(),
    );
    let dot = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
    vec_result(
        f,
        [
            v[0] - 2.0 * dot * n[0],
            v[1] - 2.0 * dot * n[1],
            v[2] - 2.0 * dot * n[2],
        ],
    )
}

// ---- Rotators ---------------------------------------------------------------

fn rot_of(frame: &Frame<'_>, args: &CallArgs, i: usize) -> Option<[i32; 3]> {
    let PropValue::Struct { fields, .. } = args.get(i)? else {
        return None;
    };
    let mut out = [0i32; 3];
    for (name, value) in fields {
        let slot = match frame.arena.names.text(*name)?.to_ascii_uppercase().as_str() {
            "PITCH" => 0,
            "YAW" => 1,
            "ROLL" => 2,
            _ => continue,
        };
        if let PropValue::Int(v) = value {
            out[slot] = *v;
        }
    }
    Some(out)
}

fn nat_add_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (u, v) = (
        rot_of(f, a, 0).unwrap_or_default(),
        rot_of(f, a, 1).unwrap_or_default(),
    );
    rot_result(
        f,
        [
            u[0].wrapping_add(v[0]),
            u[1].wrapping_add(v[1]),
            u[2].wrapping_add(v[2]),
        ],
    )
}

fn nat_add_equal_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = required_lvalue(a)?;
    let current = f.read_lvalue(&target)?;
    let PropValue::Struct { fields, .. } = &current else {
        return Err(Fail::new(
            "vm.native_argument_type",
            format!("AddEqual_RotatorRotator target is not a Rotator: {current:?}"),
        ));
    };
    let mut lhs = [0i32; 3];
    for (name, value) in fields {
        let slot = match f
            .arena
            .names
            .text(*name)
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str()
        {
            "PITCH" => 0,
            "YAW" => 1,
            "ROLL" => 2,
            _ => continue,
        };
        if let PropValue::Int(value) = value {
            lhs[slot] = *value;
        }
    }
    let rhs = rot_of(f, a, 1).unwrap_or_default();
    let value = rot_result(
        f,
        [
            lhs[0].wrapping_add(rhs[0]),
            lhs[1].wrapping_add(rhs[1]),
            lhs[2].wrapping_add(rhs[2]),
        ],
    )?;
    f.write_lvalue(&target, value.clone())?;
    Ok(value)
}

fn nat_sub_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (u, v) = (
        rot_of(f, a, 0).unwrap_or_default(),
        rot_of(f, a, 1).unwrap_or_default(),
    );
    rot_result(
        f,
        [
            u[0].wrapping_sub(v[0]),
            u[1].wrapping_sub(v[1]),
            u[2].wrapping_sub(v[2]),
        ],
    )
}

fn nat_equal_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(rot_of(f, a, 0) == rot_of(f, a, 1)))
}

fn nat_notequal_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(rot_of(f, a, 0) != rot_of(f, a, 1)))
}

fn nat_mul_rot_float(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (r, s) = (rot_of(f, a, 0).unwrap_or_default(), f_arg(a, 1));
    rot_result(
        f,
        [
            (r[0] as f32 * s) as i32,
            (r[1] as f32 * s) as i32,
            (r[2] as f32 * s) as i32,
        ],
    )
}

fn nat_mul_float_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (s, r) = (f_arg(a, 0), rot_of(f, a, 1).unwrap_or_default());
    rot_result(
        f,
        [
            (r[0] as f32 * s) as i32,
            (r[1] as f32 * s) as i32,
            (r[2] as f32 * s) as i32,
        ],
    )
}

fn nat_div_rot_float(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (r, s) = (rot_of(f, a, 0).unwrap_or_default(), f_arg(a, 1));
    if s == 0.0 {
        return rot_result(f, [0; 3]);
    }
    rot_result(
        f,
        [
            (r[0] as f32 / s) as i32,
            (r[1] as f32 / s) as i32,
            (r[2] as f32 / s) as i32,
        ],
    )
}

/// Rotate `v` by yaw/pitch/roll (right-handed, radians): yaw about Z,
/// then pitch about the rotated Y, then roll about the rotated X — the
/// UE1 `V >> Rotator` ordering with 65536 units per full turn.
fn rotate_axes(mut v: [f32; 3], yaw: f32, pitch: f32, roll: f32) -> [f32; 3] {
    const SCALE: f32 = std::f32::consts::TAU / 65536.0;
    let (cy, sy) = (yaw * SCALE).sin_cos();
    let (cp, sp) = (pitch * SCALE).sin_cos();
    let (cr, sr) = (roll * SCALE).sin_cos();
    // Yaw about world Z.
    let (x, y) = (v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy);
    v[0] = x;
    v[1] = y;
    // Pitch about the once-rotated Y.
    let (x, z) = (v[0] * cp + v[2] * sp, -v[0] * sp + v[2] * cp);
    v[0] = x;
    v[2] = z;
    // Roll about the twice-rotated X.
    let (y, z) = (v[1] * cr - v[2] * sr, v[1] * sr + v[2] * cr);
    v[1] = y;
    v[2] = z;
    v
}

/// Rotate a vector by a UE1 rotator (`V >> R`).
fn rotate_by_rotator(v: [f32; 3], rot: [i32; 3]) -> [f32; 3] {
    rotate_axes(v, rot[1] as f32, -rot[0] as f32, rot[2] as f32)
}

fn nat_rotate_vec_by_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // `>>` GreaterGreater_VectorRotator.
    let (v, r) = (
        vec_of(f, a, 0).unwrap_or_default(),
        rot_of(f, a, 1).unwrap_or_default(),
    );
    vec_result(f, rotate_by_rotator(v, r))
}

fn nat_rotate_vec_by_inv_rot(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    // `<<` LessLess_VectorRotator rotates by the INVERSE rotator: unwind
    // roll, then pitch, then yaw with negated angles.
    const SCALE: f32 = std::f32::consts::TAU / 65536.0;
    let (v, r) = (
        vec_of(f, a, 0).unwrap_or_default(),
        rot_of(f, a, 1).unwrap_or_default(),
    );
    let mut w = v;
    // Undo roll (about current X).
    let (sr, cr) = (-(r[2] as f32) * SCALE).sin_cos();
    let (y, z) = (w[1] * cr - w[2] * sr, w[1] * sr + w[2] * cr);
    w[1] = y;
    w[2] = z;
    // Undo pitch (about intermediate Y).
    let (sp, cp) = ((r[0] as f32) * SCALE).sin_cos();
    let (x, z) = (w[0] * cp + w[2] * sp, -w[0] * sp + w[2] * cp);
    w[0] = x;
    w[2] = z;
    // Undo yaw (about world Z).
    let (sy, cy) = (-(r[1] as f32) * SCALE).sin_cos();
    let (x, y) = (w[0] * cy - w[1] * sy, w[0] * sy + w[1] * cy);
    w[0] = x;
    w[1] = y;
    vec_result(f, w)
}

/// `RandRange(min, max)` — float lerp through the frame LCG.
fn nat_rand_range(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let (lo, hi) = (f_arg(a, 0), f_arg(a, 1));
    let t = (f.next_rand_u32() >> 8) as f32 / (1 << 24) as f32;
    Ok(PropValue::Float(lo + (hi - lo) * t))
}

// ---- Engine-facing effects (ScriptRuntime S3) ------------------------------

fn self_actor(f: &Frame<'_>, native: &str) -> VmResult<ObjectId> {
    f.self_id.ok_or_else(|| {
        Fail::new(
            "native.actor_self_missing",
            format!("{native} requires an executing actor"),
        )
    })
}

fn object_id(f: &Frame<'_>, a: &CallArgs, i: usize, native: &str) -> VmResult<Option<ObjectId>> {
    let Some(raw) = obj_arg(a, i) else {
        return Ok(None);
    };
    let id = if raw < 0 {
        f.resolve_imported_object(raw)?.ok_or_else(|| {
            Fail::new(
                "native.object_invalid",
                format!("{native} received null package object reference {raw}"),
            )
        })?
    } else {
        ObjectId(raw as u32)
    };
    if !f.arena.contains(id) {
        return Err(Fail::new(
            "native.object_invalid",
            format!("{native} received out-of-range object {id:?}"),
        ));
    }
    Ok(Some(id))
}

fn require_vector(f: &Frame<'_>, a: &CallArgs, i: usize, native: &str) -> VmResult<[f32; 3]> {
    vec_of(f, a, i).ok_or_else(|| {
        Fail::new(
            "native.vector_missing",
            format!("{native} requires vector argument {i}"),
        )
    })
}

fn require_rotator(f: &Frame<'_>, a: &CallArgs, i: usize, native: &str) -> VmResult<[i32; 3]> {
    rot_of(f, a, i).ok_or_else(|| {
        Fail::new(
            "native.rotator_missing",
            format!("{native} requires rotator argument {i}"),
        )
    })
}

fn actor_vector(f: &Frame<'_>, actor: ObjectId, property: &str) -> Option<[f32; 3]> {
    let name = f.arena.names.find_index(property)?;
    let PropValue::Struct { fields, .. } = f.arena.get(actor).ok()?.properties()?.get(name)? else {
        return None;
    };
    let mut out = [0.0; 3];
    for (slot, field) in ["X", "Y", "Z"].into_iter().enumerate() {
        let field_name = f.arena.names.find_index(field)?;
        out[slot] = match fields.iter().find(|(name, _)| *name == field_name)?.1 {
            PropValue::Float(v) => v,
            PropValue::Int(v) => v as f32,
            PropValue::Byte(v) => f32::from(v),
            _ => return None,
        };
    }
    Some(out)
}

fn actor_scalar(f: &Frame<'_>, actor: ObjectId, property: &str) -> f32 {
    let Some(name) = f.arena.names.find_index(property) else {
        return 0.0;
    };
    match f
        .arena
        .get(actor)
        .ok()
        .and_then(|object| object.properties())
        .and_then(|props| props.get(name))
    {
        Some(PropValue::Float(value)) => *value,
        Some(PropValue::Int(value)) => *value as f32,
        Some(PropValue::Byte(value)) => f32::from(*value),
        _ => 0.0,
    }
}

fn actor_is_hidden(f: &Frame<'_>, actor: ObjectId) -> bool {
    let Some(name) = f.arena.names.find_index("bHidden") else {
        return false;
    };
    f.arena
        .get(actor)
        .ok()
        .and_then(|object| object.properties())
        .and_then(|props| props.get(name))
        .is_some_and(PropValue::truthy)
}

fn actor_candidates(
    f: &Frame<'_>,
    a: &CallArgs,
    match_tag: Option<Name>,
    radius: Option<([f32; 3], f32, bool)>,
) -> VmResult<Vec<ObjectId>> {
    let actor_class = f.arena.find_by_path("Engine.Actor").ok_or_else(|| {
        Fail::new(
            "native.iterator_actor_class_missing",
            "Engine.Actor is unavailable for actor iteration",
        )
    })?;
    let base = object_id(f, a, 0, "iterator")?.unwrap_or(actor_class);
    if !matches!(f.arena.get(base)?.data, ObjectData::Class(_))
        || !f.arena.class_is_a(base, actor_class)?
    {
        return Err(Fail::new(
            "native.iterator_base_class_invalid",
            format!("iterator base {base:?} is not an Engine.Actor class"),
        ));
    }
    let tag_name = f.arena.names.find_index("Tag");
    let mut candidates = Vec::new();
    let mut consider = |id: ObjectId, object: &crate::arena::UObject| -> VmResult<()> {
        let Some(class) = object.class_id else {
            return Ok(());
        };
        if object.properties().is_none()
            || !f.arena.class_is_a(class, actor_class)?
            || !f.arena.class_is_a(class, base)?
        {
            return Ok(());
        }
        if let Some(tag) = match_tag
            && tag.index != NAME_NONE
            && !{
                let tag_value = object.properties().and_then(|props| {
                    tag_name.and_then(|name| props.get(name))
                }).or_else(|| {
                    let name = tag_name?;
                    f.arena.class_chain(class).ok()?.into_iter().find_map(|class_id| {
                        let default = match &f.arena.get(class_id).ok()?.data {
                            ObjectData::Class(data) => data.default_object?,
                            _ => return None,
                        };
                        f.arena.get(default).ok()?.properties()?.get(name)
                    })
                });
                matches!(
                    tag_value,
                    Some(PropValue::Name(candidate))
                        if candidate.number == tag.number
                            && (candidate == &tag
                                || f.arena.names.text(candidate.index)
                                    .zip(f.arena.names.text(tag.index))
                                    .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right)))
                )
            }
        {
            return Ok(());
        }
        if let Some((origin, limit, include_collision_radius)) = radius {
            let Some(location) = actor_vector(f, id, "Location") else {
                return Ok(());
            };
            let radius = limit
                + if include_collision_radius {
                    actor_scalar(f, id, "CollisionRadius")
                } else {
                    0.0
                };
            let dx = location[0] - origin[0];
            let dy = location[1] - origin[1];
            let dz = location[2] - origin[2];
            if radius > 0.0 && dx * dx + dy * dy + dz * dz >= radius * radius {
                return Ok(());
            }
        }
        candidates.push(id);
        Ok(())
    };
    if let Some(scope) = f.actor_scope() {
        for &id in scope {
            consider(id, f.arena.get(id)?)?;
        }
    } else {
        for (id, object) in f.arena.objects_iter() {
            consider(id, object)?;
        }
    }
    Ok(candidates)
}

fn nat_set_location(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_location")?;
    let location = require_vector(f, a, 0, "set_location")?;
    f.effect(crate::vm::ScriptEffect::SetLocation { actor, location })?;
    Ok(PropValue::Bool(true))
}

fn nat_set_rotation(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_rotation")?;
    let rotation = require_rotator(f, a, 0, "set_rotation")?;
    f.effect(crate::vm::ScriptEffect::SetRotation { actor, rotation })?;
    Ok(PropValue::Bool(true))
}

fn nat_set_physics(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_physics")?;
    f.effect(crate::vm::ScriptEffect::SetPhysics {
        actor,
        physics: i_arg(a, 0),
    })?;
    Ok(PropValue::Int(0))
}

fn nat_set_collision(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_collision")?;
    f.effect(crate::vm::ScriptEffect::SetCollision {
        actor,
        colliding_actors: b_arg(a, 0),
        block_actors: b_arg(a, 1),
        block_players: b_arg(a, 2),
    })?;
    Ok(PropValue::Int(0))
}

fn nat_set_collision_size(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_collision_size")?;
    let radius = f_arg(a, 0);
    f.effect(crate::vm::ScriptEffect::SetCollisionSize {
        actor,
        radius,
        height: f_arg(a, 1),
        width: a.get(2).map(|_| f_arg(a, 2)).unwrap_or(radius),
    })?;
    Ok(PropValue::Bool(true))
}

fn nat_set_owner(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_owner")?;
    f.effect(crate::vm::ScriptEffect::SetOwner {
        actor,
        owner: object_id(f, a, 0, "set_owner")?,
    })?;
    Ok(PropValue::Int(0))
}

fn nat_set_base(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "set_base")?;
    f.effect(crate::vm::ScriptEffect::SetBase {
        actor,
        base: object_id(f, a, 0, "set_base")?,
    })?;
    Ok(PropValue::Int(0))
}
/// Native 512 (`AActor::MakeNoise`) only captures the sender and authored
/// loudness. The engine owns recipient selection and `HearNoise` dispatch.
fn nat_make_noise(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "make_noise")?;
    f.effect(crate::vm::ScriptEffect::MakeNoise {
        actor,
        loudness: f_arg(a, 0),
    })?;
    Ok(PropValue::Int(0))
}

/// Native 329 (`AActor::IsSoftwareRendering`), from
/// `UnScript.cpp:2873-2882`, compares
/// `Engine.Engine/GameRenderDevice` with `SoftDrv.SoftwareRenderDevice`
/// case-insensitively. hp2's renderer is always the Rust Vulkan/wgpu backend,
/// so the native result is determined by that backend rather than host config.
fn nat_is_software_rendering(_f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Bool(false))
}

/// `UObject::execLocalize` is registered with `INDEX_NONE`, so it reaches
/// this body through the unnumbered-native path. File/config access remains
/// at the engine effect boundary.
fn nat_localize(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let request_id = f.next_effect_request_id();
    f.set_next_effect_request_id(request_id.wrapping_add(1));
    f.effect(crate::vm::ScriptEffect::LocalizeRequest {
        request_id,
        section: s_arg(a, 0),
        key: s_arg(a, 1),
        package: s_arg(a, 2),
    })?;
    f.await_effect_result(request_id);
    Ok(PropValue::Int(0))
}

#[cfg(test)]
fn is_software_render_device(configured_device: &str) -> bool {
    configured_device.eq_ignore_ascii_case("SoftDrv.SoftwareRenderDevice")
}

/// Native 529 (`APawn::AddPawn`), matching `UnPawn.cpp:1105-1113`.
/// The engine performs the ordered world mutation at the effect boundary.
fn nat_add_pawn(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    let pawn = self_actor(f, "add_pawn")?;
    let pawn_class = f.arena.find_by_path("Engine.Pawn").ok_or_else(|| {
        Fail::new(
            "native.add_pawn_class_missing",
            "Engine.Pawn class is unavailable",
        )
    })?;
    let self_class = f.arena.get(pawn)?.class_id.ok_or_else(|| {
        Fail::new(
            "native.add_pawn_self_invalid",
            format!("AddPawn self {pawn:?} has no class"),
        )
    })?;
    if !f.arena.class_is_a(self_class, pawn_class)? {
        return Err(Fail::new(
            "native.add_pawn_self_invalid",
            format!("AddPawn self {pawn:?} is not a Pawn"),
        ));
    }
    f.effect(crate::vm::ScriptEffect::AddPawn { pawn })?;
    Ok(PropValue::Int(0))
}

/// `Spawn` is a result-bearing engine effect. The reserved append-only handle
/// is safe to propagate through this slice's overlays/effects, but the frame
/// suspends at the native boundary so the engine allocates it before the
/// activation continues into a later opcode.
fn nat_spawn(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let class = object_id(f, a, 0, "spawn")?.ok_or_else(|| {
        Fail::new(
            "native.spawn_class_missing",
            format!(
                "Spawn requires a class<Actor> argument; evaluated argument={:?}",
                a.get(0)
            ),
        )
    })?;
    let actor_class = f.arena.find_by_path("Engine.Actor").ok_or_else(|| {
        Fail::new(
            "native.spawn_actor_class_missing",
            "Engine.Actor class is unavailable",
        )
    })?;
    if !matches!(f.arena.get(class)?.data, ObjectData::Class(_))
        || !f.arena.class_is_a(class, actor_class)?
    {
        return Err(Fail::new(
            "native.spawn_class_invalid",
            format!("Spawn class {class:?} is not a class derived from Engine.Actor"),
        ));
    }
    let owner = object_id(f, a, 1, "spawn")?;
    let location = vec_of(f, a, 3)
        .or_else(|| {
            f.self_id
                .and_then(|actor| actor_vector(f, actor, "Location"))
        })
        .unwrap_or([0.0; 3]);
    let rotation = rot_of(f, a, 4).unwrap_or([0; 3]);
    let (request_id, reserved) = f.reserve_effect_object()?;
    f.effect(crate::vm::ScriptEffect::SpawnRequest {
        request_id,
        reserved,
        class,
        owner,
        location,
        rotation,
    })?;
    f.await_effect_result(request_id);
    // This value is never observable: the enclosing expression is serialized
    // and resumes only after `Frame::inject_effect_result`.
    Ok(PropValue::Int(0))
}

fn nat_destroy(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "destroy")?;
    f.effect(crate::vm::ScriptEffect::Destroy { actor })?;
    Ok(PropValue::Bool(true))
}

fn nat_play_sound(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "play_sound")?;
    let Some(cue) = object_id(f, a, 0, "play_sound")? else {
        f.effect(crate::vm::ScriptEffect::AudioUnavailable {
            actor: Some(actor),
            source: "PlaySound None".to_string(),
        })?;
        return Ok(PropValue::Int(0));
    };
    f.effect(crate::vm::ScriptEffect::PlaySound { actor, cue })?;
    Ok(PropValue::Int(0))
}

fn nat_stop_sound(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    f.effect(crate::vm::ScriptEffect::StopSound {
        actor: self_actor(f, "stop_sound")?,
        cue: object_id(f, a, 0, "stop_sound")?,
    })?;
    Ok(PropValue::Int(0))
}
fn nat_play_music(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let song = s_arg(a, 0);
    if song.is_empty() {
        return Ok(PropValue::Int(0));
    }
    // ALAudio returns an opaque non-zero song handle. Derive one from the
    // authored song key so repeated starts are stable without putting audio
    // device state into the deterministic VM.
    let mut hash = 0x811c_9dc5u32;
    for byte in song.bytes() {
        hash = (hash ^ u32::from(byte.to_ascii_lowercase())).wrapping_mul(0x0100_0193);
    }
    let handle = (hash & 0x7fff_ffff).max(1) as i32;
    f.effect(crate::vm::ScriptEffect::PlayMusic {
        actor: self_actor(f, "play_music")?,
        song,
        handle,
    })?;
    Ok(PropValue::Int(handle))
}

fn nat_stop_music(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = self_actor(f, "stop_music")?;
    let handle = i_arg(a, 0);
    if handle <= 0 {
        f.effect(crate::vm::ScriptEffect::AudioUnavailable {
            actor: Some(actor),
            source: format!("StopMusic invalid/unstarted handle {handle}"),
        })?;
    } else {
        f.effect(crate::vm::ScriptEffect::StopMusic { actor, handle })?;
    }
    Ok(PropValue::Int(0))
}

fn nat_stop_all_music(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    f.effect(crate::vm::ScriptEffect::StopAllMusic {
        actor: self_actor(f, "stop_all_music")?,
    })?;
    Ok(PropValue::Int(0))
}

fn anim_name(a: &CallArgs) -> Name {
    match a.get(0) {
        Some(PropValue::Name(name)) => *name,
        _ => Name::none(),
    }
}

fn nat_skeletal_query(
    f: &mut Frame<'_>,
    query: crate::vm::SkeletalQuery,
    placeholder: PropValue,
    native: &str,
) -> Result<PropValue> {
    let request_id = f.reserve_effect_request()?;
    f.effect(crate::vm::ScriptEffect::SkeletalQuery {
        request_id,
        actor: self_actor(f, native)?,
        query,
    })?;
    f.await_effect_result(request_id);
    Ok(placeholder)
}

fn nat_has_anim(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_skeletal_query(
        f,
        crate::vm::SkeletalQuery::HasAnim {
            sequence: anim_name(a),
        },
        PropValue::Bool(false),
        "HasAnim",
    )
}

fn nat_bone_number(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_skeletal_query(
        f,
        crate::vm::SkeletalQuery::BoneNumber { bone: anim_name(a) },
        PropValue::Int(0),
        "BoneNumber",
    )
}

fn nat_bone_name(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_skeletal_query(
        f,
        crate::vm::SkeletalQuery::BoneName { index: i_arg(a, 0) },
        PropValue::Name(Name::none()),
        "BoneName",
    )
}

fn nat_is_animating(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_skeletal_query(
        f,
        crate::vm::SkeletalQuery::IsAnimating {
            root_bone: anim_name(a),
        },
        PropValue::Bool(false),
        "IsAnimating",
    )
}

fn nat_get_anim_group(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_skeletal_query(
        f,
        crate::vm::SkeletalQuery::AnimGroup {
            sequence: anim_name(a),
        },
        PropValue::Name(Name::none()),
        "GetAnimGroup",
    )
}


fn nat_create_anim_channel(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let class = object_id(f, a, 0, "CreateAnimChannel")?;
    let root_bone = match a.get(2) {
        Some(PropValue::Name(name)) => *name,
        _ => Name::none(),
    };
    let (request_id, reserved) = f.reserve_effect_object()?;
    f.effect(crate::vm::ScriptEffect::CreateAnimChannelRequest {
        request_id,
        reserved,
        actor: self_actor(f, "CreateAnimChannel")?,
        class,
        anim_type: i_arg(a, 1),
        root_bone,
        transient: b_arg(a, 3),
        not_replaceable: b_arg(a, 4),
    })?;
    f.await_effect_result(request_id);
    Ok(PropValue::Object(None))
}

fn nat_play_anim(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let sequence = anim_name(a);
    if !sequence.is_none() {
        f.effect(crate::vm::ScriptEffect::PlayAnim {
            actor: self_actor(f, "play_anim")?,
            sequence,
            rate: f_arg(a, 1),
            looped: false,
        })?;
    }
    Ok(PropValue::Int(0))
}

fn nat_loop_anim(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let sequence = anim_name(a);
    if !sequence.is_none() {
        f.effect(crate::vm::ScriptEffect::LoopAnim {
            actor: self_actor(f, "loop_anim")?,
            sequence,
            rate: f_arg(a, 1),
            looped: true,
        })?;
    }
    Ok(PropValue::Int(0))
}

fn nat_tween_anim(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let sequence = anim_name(a);
    if !sequence.is_none() {
        f.effect(crate::vm::ScriptEffect::TweenAnim {
            actor: self_actor(f, "tween_anim")?,
            sequence,
            rate: f_arg(a, 1),
            looped: false,
        })?;
    }
    Ok(PropValue::Int(0))
}

fn nat_move(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    f.effect(crate::vm::ScriptEffect::MoveSmooth {
        actor: self_actor(f, "move")?,
        delta: require_vector(f, a, 0, "move")?,
    })?;
    Ok(PropValue::Bool(true))
}

fn nat_move_smooth(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    f.effect(crate::vm::ScriptEffect::MoveSmooth {
        actor: self_actor(f, "move_smooth")?,
        delta: require_vector(f, a, 0, "move_smooth")?,
    })?;
    Ok(PropValue::Bool(true))
}

fn nat_move_to(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    f.effect(crate::vm::ScriptEffect::MoveTo {
        actor: self_actor(f, "move_to")?,
        destination: require_vector(f, a, 0, "move_to")?,
    })?;
    Ok(PropValue::Int(0))
}

fn nat_move_toward(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = object_id(f, a, 0, "move_toward")?.ok_or_else(|| {
        Fail::new(
            "native.move_toward_target_missing",
            "MoveToward requires an actor target",
        )
    })?;
    f.effect(crate::vm::ScriptEffect::MoveToward {
        actor: self_actor(f, "move_toward")?,
        target,
    })?;
    Ok(PropValue::Int(0))
}

fn nat_turn_to(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    f.effect(crate::vm::ScriptEffect::TurnTo {
        actor: self_actor(f, "turn_to")?,
        focus: require_vector(f, a, 0, "turn_to")?,
    })?;
    Ok(PropValue::Int(0))
}

fn nat_turn_toward(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = object_id(f, a, 0, "turn_toward")?.ok_or_else(|| {
        Fail::new(
            "native.turn_toward_target_missing",
            "TurnToward requires an actor target",
        )
    })?;
    f.effect(crate::vm::ScriptEffect::TurnToward {
        actor: self_actor(f, "turn_toward")?,
        target,
    })?;
    Ok(PropValue::Int(0))
}

fn nat_trigger_event(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let name = anim_name(a);
    if !name.is_none() {
        f.effect(crate::vm::ScriptEffect::TriggerEvent {
            name,
            other: object_id(f, a, 1, "trigger_event")?,
            instigator: object_id(f, a, 2, "trigger_event")?,
        })?;
    }
    Ok(PropValue::Int(0))
}

fn nat_all_actors(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let tag = match a.get(2) {
        Some(PropValue::Name(name)) => Some(*name),
        _ => None,
    };
    let values = actor_candidates(f, a, tag, None)?;
    f.begin_iterator(a, 1, values)?;
    Ok(PropValue::Object(None))
}

fn nat_radius_actors(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let origin = vec_of(f, a, 3)
        .or_else(|| {
            self_actor(f, "radius_actors")
                .ok()
                .and_then(|id| actor_vector(f, id, "Location"))
        })
        .ok_or_else(|| {
            Fail::new(
                "native.radius_actors_origin_missing",
                "RadiusActors has no origin",
            )
        })?;
    let values = actor_candidates(f, a, None, Some((origin, f_arg(a, 2), true)))?;
    f.begin_iterator(a, 1, values)?;
    Ok(PropValue::Object(None))
}

fn nat_visible_actors(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let origin = vec_of(f, a, 3)
        .or_else(|| {
            self_actor(f, "visible_actors")
                .ok()
                .and_then(|id| actor_vector(f, id, "Location"))
        })
        .ok_or_else(|| {
            Fail::new(
                "native.visible_actors_origin_missing",
                "VisibleActors has no origin",
            )
        })?;
    let radius = f_arg(a, 2);
    let mut values = actor_candidates(
        f,
        a,
        None,
        (radius > 0.0).then_some((origin, radius, false)),
    )?;
    values.retain(|actor| !actor_is_hidden(f, *actor));
    f.begin_iterator(a, 1, values)?;
    Ok(PropValue::Object(None))
}

/// Native 309 (`AActor::TraceActors`). The engine owns collision geometry,
/// so the iterator parks once while that geometry is queried, then resumes
/// with the complete ordered hit list and all three out bindings intact.
fn nat_trace_actors(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let source = self_actor(f, "trace_actors")?;
    let end = require_vector(f, a, 4, "trace_actors")?;
    let start = vec_of(f, a, 5)
        .or_else(|| actor_vector(f, source, "Location"))
        .ok_or_else(|| {
            Fail::new(
                "native.trace_collision_unavailable",
                format!("TraceActors source {source:?} has no Location for range to {end:?}"),
            )
        })?;
    let extent = vec_of(f, a, 6).unwrap_or([0.0; 3]);
    f.begin_deferred_trace_iterator(a, 1, 2, 3)?;
    let request_id = f.reserve_effect_request()?;
    f.effect(crate::vm::ScriptEffect::TraceActorsRequest {
        request_id,
        source,
        base_class: object_id(f, a, 0, "trace_actors")?,
        start,
        end,
        extent,
    })?;
    f.await_effect_result(request_id);
    Ok(PropValue::Array(Vec::new()))
}

/// Native 548 (`AActor::FastTrace`). This is a world-model query rather than
/// a frame-local approximation, so it suspends until the engine resolves the
/// segment. As in the UE1 ABI, an omitted `TraceStart` means `Location`.
fn nat_fast_trace(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let source = self_actor(f, "fast_trace")?;
    let end = require_vector(f, a, 0, "fast_trace")?;
    let start = vec_of(f, a, 1)
        .or_else(|| actor_vector(f, source, "Location"))
        .ok_or_else(|| {
            Fail::new(
                "native.trace_collision_unavailable",
                format!("FastTrace source {source:?} has no Location for range to {end:?}"),
            )
        })?;
    let request_id = f.reserve_effect_request()?;
    f.effect(crate::vm::ScriptEffect::FastTraceRequest {
        request_id,
        source,
        start,
        end,
    })?;
    f.await_effect_result(request_id);
    Ok(PropValue::Bool(false))
}

/// Native 277 (`AActor::Trace`). Collision and LevelInfo ownership live in
/// the engine; capture its two out references before suspending for the
/// earliest world or optional actor hit.
fn nat_trace(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let source = self_actor(f, "trace")?;
    let end = require_vector(f, a, 2, "trace")?;
    let start = vec_of(f, a, 3)
        .or_else(|| actor_vector(f, source, "Location"))
        .ok_or_else(|| {
            Fail::new(
                "native.trace_collision_unavailable",
                format!("Trace source {source:?} has no Location for range to {end:?}"),
            )
        })?;
    f.begin_deferred_trace(a, 0, 1)?;
    let request_id = f.reserve_effect_request()?;
    f.effect(crate::vm::ScriptEffect::TraceRequest {
        request_id,
        source,
        start,
        end,
        trace_actors: b_arg(a, 4),
        extent: vec_of(f, a, 5).unwrap_or([0.0; 3]),
    })?;
    f.await_effect_result(request_id);
    Ok(PropValue::Object(None))
}

/// The pure-native table. Keys are case-folded leaf function names as they
/// appear in the shipped packages (verified against the PrivetDr census:
/// every numbered slot below 0x100 that the bytecode actually calls and
/// that needs no out-parameter write-back has an entry here).
// ---- State machine + latents (ScriptRuntime S2) -----------------------------

/// Text payload of a Name argument (`None` for absent/null arguments).
fn name_arg(f: &Frame<'_>, a: &CallArgs, i: usize) -> Option<String> {
    match a.get(i) {
        Some(PropValue::Name(n)) => f.arena.names.text(n.index).map(str::to_string),
        _ => None,
    }
}

/// `GotoState([name])` — queue a state switch for the embedding scheduler.
/// Omitted, `None`, and `Auto` have distinct engine meanings, so retain
/// that identity in [`crate::vm::StateChange`] instead of collapsing them
/// into an optional string.
fn nat_goto_state(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let target = match a.get(0) {
        None => crate::vm::StateChange::Current,
        Some(PropValue::Name(name)) => match f.arena.names.text(name.index) {
            Some(text) if crate::name::fold_key(text) == "auto" => crate::vm::StateChange::Auto,
            Some(text)
                if crate::name::fold_key(text).is_empty()
                    || crate::name::fold_key(text) == "none" =>
            {
                crate::vm::StateChange::None
            }
            Some(text) => crate::vm::StateChange::Named(text.to_string()),
            None => {
                return Err(Fail::new(
                    "vm.state_name_invalid",
                    format!("GotoState received missing name index {}", name.index),
                ));
            }
        },
        Some(other) => {
            return Err(Fail::new(
                "vm.state_name_invalid",
                format!("GotoState expects a name, received {other:?}"),
            ));
        }
    };
    let actor = f
        .self_id
        .ok_or_else(|| Fail::new("vm.state_self_missing", "GotoState has no self actor"))?;
    f.request(crate::vm::LatentRequest::GotoState {
        actor,
        change: target,
    })?;
    // State changes replace this activation's state frame in UE1. Stop at
    // the native call boundary so no later bytecode can schedule timers or
    // writes from the abandoned state.
    f.suspend_for(crate::vm::Suspend::StateChange);
    Ok(PropValue::Int(0))
}

/// `GetStateName()` — the name index of the state whose code (or actor
/// context) this frame runs in, as declared by the embedder. Outside any
/// state this is NAME_None.
fn nat_get_state_name(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    Ok(PropValue::Name(
        f.arena.name_handle(f.active_state().unwrap_or(0)),
    ))
}

/// `IsInState(name)` — case-folded comparison against the embedder's
/// active-state declaration. Known gap, documented loudly here:
/// comparing against `'None'` reports false rather than detecting
/// "no active state", because NAME_None and a real `None` state are
/// indistinguishable at the value level this native receives.
fn nat_is_in_state(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let wanted = name_arg(f, a, 0);
    let current = f
        .active_state()
        .and_then(|idx| f.arena.names.text(idx))
        .map(crate::name::fold_key);
    let hit = match (&wanted, current) {
        (Some(w), Some(c)) => crate::name::fold_key(w) == c,
        _ => false,
    };
    Ok(PropValue::Bool(hit))
}

/// `SetTimer(Seconds, bRepeating)` — schedule the actor's `Timer` event
/// with the embedding scheduler's timer wheel.
fn nat_set_timer(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    let actor = f
        .self_id
        .ok_or_else(|| Fail::new("vm.timer_self_missing", "SetTimer has no self actor"))?;
    let seconds = f
        .arg_named(a, "NewTimerRate")
        .map(f_value)
        .unwrap_or_else(|| f_arg(a, 0));
    let repeating = f
        .arg_named(a, "bLoop")
        .map(PropValue::truthy)
        .unwrap_or_else(|| b_arg(a, 1));
    f.request(crate::vm::LatentRequest::SetTimer {
        actor,
        seconds,
        repeating,
    })?;
    Ok(PropValue::Int(0))
}

/// `Sleep(Seconds)` — park the frame until `Seconds` of simulated time
/// elapse; `Frame::run` stops just past this call.
fn nat_sleep(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    f.suspend_for(crate::vm::Suspend::Sleep(f64::from(f_arg(a, 0)).max(0.0)));
    Ok(PropValue::Int(0))
}

/// `FinishAnim` — park until an animation completes. Animation playback
/// lands with PSA decoding later; the scheduler wakes these frames after
/// its documented placeholder duration.
fn nat_finish_anim(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    f.suspend_for(crate::vm::Suspend::Anim);
    Ok(PropValue::Int(0))
}

/// `FinishInterpolation` — park until the actor's interpolation flag clears.
/// The engine scheduler polls the flag; no duration approximation is valid.
fn nat_finish_interpolation(f: &mut Frame<'_>, _a: &CallArgs) -> Result<PropValue> {
    f.suspend_for(crate::vm::Suspend::Interpolation);
    Ok(PropValue::Int(0))
}

/// `Enable(name ProbeFunc)` / `Disable(name ProbeFunc)`: alter dispatch of
/// a probe event on the actor's current state frame. The engine-side
/// scheduler owns that mutable state, so the native queues a request after
/// validating the call-frame name argument.
fn nat_set_event_enabled(f: &mut Frame<'_>, a: &CallArgs, enabled: bool) -> Result<PropValue> {
    let event = name_arg(f, a, 0).ok_or_else(|| {
        Fail::new(
            "vm.probe_name_invalid",
            "Enable/Disable requires a readable probe-function name",
        )
    })?;
    let actor = f
        .self_id
        .ok_or_else(|| Fail::new("vm.probe_self_missing", "Enable/Disable has no self actor"))?;
    f.request(crate::vm::LatentRequest::SetEventEnabled {
        actor,
        event,
        enabled,
    })?;
    Ok(PropValue::Int(0))
}

fn nat_enable(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_set_event_enabled(f, a, true)
}

fn nat_disable(f: &mut Frame<'_>, a: &CallArgs) -> Result<PropValue> {
    nat_set_event_enabled(f, a, false)
}

static TABLE: &[(&str, NativeImpl)] = &[
    // Int
    ("add_intint", nat_add_int),
    ("subtract_intint", nat_sub_int),
    ("multiply_intint", nat_mul_int),
    ("divide_intint", nat_div_int),
    ("less_intint", nat_less_int),
    ("greater_intint", nat_greater_int),
    ("lessequal_intint", nat_lessequal_int),
    ("greaterequal_intint", nat_greaterequal_int),
    ("equalequal_intint", nat_equal_int),
    ("notequal_intint", nat_notequal_int),
    ("and_intint", nat_and_int),
    ("or_intint", nat_or_int),
    ("xor_intint", nat_xor_int),
    ("complement_preint", nat_complement_int),
    ("lessless_intint", nat_shl_int),
    ("greatergreater_intint", nat_shr_int),
    ("greatergreatergreater_intint", nat_shr_logical_int),
    ("subtract_preint", nat_neg_int),
    ("min", nat_min_int),
    ("max", nat_max_int),
    ("clamp", nat_clamp_int),
    ("abs", nat_abs_float),
    ("rand", nat_rand),
    ("addequal_intint", nat_add_equal_int),
    ("addadd_preint", nat_preincrement_int),
    ("subtractsubtract_preint", nat_predecrement_int),
    ("subtractsubtract_int", nat_postdecrement_int),
    ("addadd_int", nat_postincrement_int),
    // Float
    ("add_floatfloat", nat_add_float),
    ("subtract_floatfloat", nat_sub_float),
    ("multiply_floatfloat", nat_mul_float),
    ("divide_floatfloat", nat_div_float),
    ("multiplymultiply_floatfloat", nat_pow_float),
    ("percent_floatfloat", nat_mod_float),
    ("less_floatfloat", nat_less_float),
    ("greater_floatfloat", nat_greater_float),
    ("lessequal_floatfloat", nat_lessequal_float),
    ("greaterequal_floatfloat", nat_greaterequal_float),
    ("equalequal_floatfloat", nat_equal_float),
    ("notequal_floatfloat", nat_notequal_float),
    ("complementequal_floatfloat", nat_complement_equal_float),
    ("addequal_floatfloat", nat_add_equal_float),
    ("multiplyequal_floatfloat", nat_multiply_equal_float),
    ("subtract_prefloat", nat_neg_float),
    ("sin", nat_sin),
    ("cos", nat_cos),
    ("tan", nat_tan),
    ("atan", nat_atan),
    ("exp", nat_exp),
    ("loge", nat_loge),
    ("sqrt", nat_sqrt),
    ("square", nat_square),
    ("fmin", nat_fmin),
    ("fmax", nat_fmax),
    ("fclamp", nat_fclamp),
    ("lerp", nat_lerp),
    ("smerp", nat_smerp),
    ("frand", nat_frand),
    ("vrand", nat_vrand),
    ("rotrand", nat_rotrand),
    ("randrange", nat_rand_range),
    // Bool / Name / Object
    ("not_prebool", nat_not_bool),
    ("andand_boolbool", nat_andand_bool),
    ("oror_boolbool", nat_oror_bool),
    ("xorxor_boolbool", nat_xorxor_bool),
    ("equalequal_boolbool", nat_equal_bool),
    ("notequal_boolbool", nat_notequal_bool),
    ("equalequal_namename", nat_equal_name),
    ("notequal_namename", nat_notequal_name),
    ("equalequal_objectobject", nat_equal_object),
    ("notequal_objectobject", nat_notequal_object),
    // Strings
    ("concat_strstr", nat_concat_str),
    ("at_strstr", nat_at_str),
    ("len", nat_len),
    ("instr", nat_instr),
    ("mid", nat_mid),
    ("left", nat_left),
    ("right", nat_right),
    ("caps", nat_caps),
    ("chr", nat_chr),
    ("asc", nat_asc),
    ("equalequal_strstr", nat_equal_str),
    ("notequal_strstr", nat_notequal_str),
    ("complementequal_strstr", nat_complement_equal_str),
    // Vectors
    ("add_vectorvector", nat_add_vec),
    ("addequal_vectorvector", nat_add_equal_vec),
    ("subtract_vectorvector", nat_sub_vec),
    ("subtract_prevector", nat_neg_vec),
    ("multiply_vectorvector", nat_mul_vec_vec),
    ("multiply_vectorfloat", nat_mul_vec_float),
    ("multiply_floatvector", nat_mul_float_vec),
    ("divide_vectorfloat", nat_div_vec_float),
    ("equalequal_vectorvector", nat_equal_vec),
    ("notequal_vectorvector", nat_notequal_vec),
    ("dot_vectorvector", nat_dot),
    ("cross_vectorvector", nat_cross),
    ("vsize", nat_vsize),
    ("normal", nat_normal),
    ("mirrorvectorbynormal", nat_mirror_vec_by_normal),
    // Rotators
    ("add_rotatorrotator", nat_add_rot),
    ("addequal_rotatorrotator", nat_add_equal_rot),
    ("subtract_rotatorrotator", nat_sub_rot),
    ("equalequal_rotatorrotator", nat_equal_rot),
    ("notequal_rotatorrotator", nat_notequal_rot),
    ("multiply_rotatorfloat", nat_mul_rot_float),
    ("multiply_floatrotator", nat_mul_float_rot),
    ("divide_rotatorfloat", nat_div_rot_float),
    ("greatergreater_vectorrotator", nat_rotate_vec_by_rot),
    ("lessless_vectorrotator", nat_rotate_vec_by_inv_rot),
    // Diagnostics / misc
    ("log", nat_log),
    ("warn", nat_warn),
    ("error", nat_actor_error),
    ("saveconfig", nat_save_config),
    ("staticsaveconfig", nat_save_config),
    ("resetconfig", nat_save_config),
    ("dynamicloadobject", nat_dynamic_load_object),
    ("getsoundduration", nat_get_sound_duration),
    ("isa", nat_is_a),
    ("classischildof", nat_class_is_child_of),
    ("localize", nat_localize),
    // Engine effects + actor iterators (ScriptRuntime S3).
    ("setlocation", nat_set_location),
    ("setrotation", nat_set_rotation),
    ("setphysics", nat_set_physics),
    ("setcollision", nat_set_collision),
    ("setcollisionsize", nat_set_collision_size),
    ("setowner", nat_set_owner),
    ("setbase", nat_set_base),
    ("makenoise", nat_make_noise),
    ("issoftwarerendering", nat_is_software_rendering),
    ("addpawn", nat_add_pawn),
    ("spawn", nat_spawn),
    ("destroy", nat_destroy),
    ("playsound", nat_play_sound),
    ("stopsound", nat_stop_sound),
    ("playmusic", nat_play_music),
    ("stopmusic", nat_stop_music),
    ("stopallmusic", nat_stop_all_music),
    ("playanim", nat_play_anim),
    ("loopanim", nat_loop_anim),
    ("tweenanim", nat_tween_anim),
    ("hasanim", nat_has_anim),
    ("bonenumber", nat_bone_number),
    ("bonename", nat_bone_name),
    ("isanimating", nat_is_animating),
    ("getanimgroup", nat_get_anim_group),
    ("createanimchannel", nat_create_anim_channel),
    ("move", nat_move),
    ("movesmooth", nat_move_smooth),
    ("moveto", nat_move_to),
    ("movetoward", nat_move_toward),
    ("turnto", nat_turn_to),
    ("turntoward", nat_turn_toward),
    ("triggerevent", nat_trigger_event),
    ("allactors", nat_all_actors),
    ("radiusactors", nat_radius_actors),
    ("visibleactors", nat_visible_actors),
    ("traceactors", nat_trace_actors),
    ("trace", nat_trace),
    ("fasttrace", nat_fast_trace),
    // State machine + latents (ScriptRuntime S2): these hand their effect
    // to the embedding scheduler — see `crate::vm::{Suspend,
    // LatentRequest}` and hp-engine's per-actor frame scheduler.
    ("gotostate", nat_goto_state),
    ("enable", nat_enable),
    ("disable", nat_disable),
    ("getstatename", nat_get_state_name),
    ("isinstate", nat_is_in_state),
    ("settimer", nat_set_timer),
    ("sleep", nat_sleep),
    ("finishanim", nat_finish_anim),
    ("finishinterpolation", nat_finish_interpolation),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::{
        BytecodeResolver, ClassData, ObjectArena, ObjectData, PropertyData, UObject,
    };
    use crate::natives::{CallArg, NativeRegistry};
    use crate::value::PropertyKind;

    fn args(values: &[PropValue]) -> CallArgs {
        let mut out = CallArgs::default();
        for v in values {
            out.push(CallArg {
                name: None,
                value: v.clone(),
                lvalue: None,
            });
        }
        out
    }

    fn empty_frame<'a>(arena: &'a ObjectArena) -> Frame<'a> {
        static REGISTRY: std::sync::OnceLock<NativeRegistry> = std::sync::OnceLock::new();
        Frame::new(arena, REGISTRY.get_or_init(NativeRegistry::new), &[], None)
    }
    #[test]
    fn software_rendering_uses_backend_decision_not_host_config() {
        assert!(is_software_render_device("SoftDrv.SoftwareRenderDevice"));
        assert!(is_software_render_device("softdrv.softwarerenderdevice"));
        assert!(!is_software_render_device("D3DDrv.D3DRenderDevice"));

        let arena = ObjectArena::new();
        let mut frame = empty_frame(&arena);
        assert_eq!(
            eval(&mut frame, "IsSoftwareRendering", &CallArgs::default()),
            PropValue::Bool(false),
            "the Rust Vulkan/wgpu backend is never the legacy software device"
        );
    }

    #[test]
    fn add_pawn_validates_ancestry_and_queues_void_effect() {
        let mut arena = ObjectArena::new();
        let engine = arena.root_for("Engine");
        let pawn_name = arena.names.intern("Pawn");
        let pawn_class = arena.alloc(UObject {
            name_index: pawn_name,
            outer: Some(engine),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let pawn_instance_name = arena.names.intern("TestPawn");
        let pawn = arena.alloc(UObject {
            class_id: Some(pawn_class),
            name_index: pawn_instance_name,
            outer: Some(engine),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let registry = NativeRegistry::new();
        let mut frame = Frame::new(&arena, &registry, &[], Some(pawn));
        assert_eq!(
            eval(&mut frame, "AddPawn", &CallArgs::default()),
            PropValue::Int(0)
        );
        assert_eq!(
            frame.take_effects(),
            vec![crate::vm::ScriptEffect::AddPawn { pawn }]
        );
    }
    #[test]
    fn spawn_accepts_only_actor_class_objects() {
        let mut arena = ObjectArena::new();
        let engine = arena.root_for("Engine");
        let actor_name = arena.names.intern("Actor");
        let actor_class = arena.alloc(UObject {
            name_index: actor_name,
            outer: Some(engine),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let shadow_name = arena.names.intern("TestShadow");
        let shadow_class = arena.alloc(UObject {
            name_index: shadow_name,
            outer: Some(engine),
            data: ObjectData::Class(ClassData {
                super_class: Some(actor_class),
                ..Default::default()
            }),
            ..Default::default()
        });
        let non_class_name = arena.names.intern("NotAClass");
        let non_class = arena.alloc(UObject {
            name_index: non_class_name,
            outer: Some(engine),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let registry = NativeRegistry::new();
        let mut frame = Frame::new(&arena, &registry, &[], None);
        nat_spawn(
            &mut frame,
            &args(&[PropValue::Object(Some(shadow_class.0 as i32))]),
        )
        .expect("Actor-derived class");
        assert!(matches!(
            frame.take_effects().as_slice(),
            [crate::vm::ScriptEffect::SpawnRequest { class, .. }] if *class == shadow_class
        ));

        // Authored class properties retain signed linker imports. Native
        // dispatch resolves them through the executing function's package
        // tables before applying the same UClass/Actor ancestry checks.
        let resolver = BytecodeResolver::from_package_refs(
            vec![Some(shadow_class), Some(non_class)],
            Vec::new(),
        );
        let mut imported = Frame::with_resolver(&arena, &registry, &[], None, resolver.clone());
        nat_spawn(&mut imported, &args(&[PropValue::Object(Some(-1))]))
            .expect("imported Actor-derived class");
        assert!(matches!(
            imported.take_effects().as_slice(),
            [crate::vm::ScriptEffect::SpawnRequest { class, .. }] if *class == shadow_class
        ));
        let mut imported_invalid = Frame::with_resolver(&arena, &registry, &[], None, resolver);
        let fail = nat_spawn(&mut imported_invalid, &args(&[PropValue::Object(Some(-2))]))
            .expect_err("imported ordinary object must remain invalid");
        assert_eq!(fail.reason_code, "native.spawn_class_invalid");

        let mut invalid = Frame::new(&arena, &registry, &[], None);
        let fail = nat_spawn(
            &mut invalid,
            &args(&[PropValue::Object(Some(non_class.0 as i32))]),
        )
        .expect_err("ordinary object must not be coerced into a class");
        assert_eq!(fail.reason_code, "native.spawn_class_invalid");
    }

    /// Arena carrying Core.Vector / Core.Rotator templates.
    fn core_arena() -> ObjectArena {
        let mut arena = ObjectArena::new();
        // Package roots live in the root map only when made via root_for.
        let root = arena.root_for("Core");
        for (name, fields) in [
            ("Vector", &["X", "Y", "Z"][..]),
            ("Rotator", &["Pitch", "Yaw", "Roll"][..]),
        ] {
            let mut children = Vec::new();
            for field in fields {
                let index = arena.names.intern(field);
                children.push(arena.alloc(UObject {
                    name_index: index,
                    outer: Some(root),
                    data: ObjectData::Property(Box::new(PropertyData {
                        kind: PropertyKind::Float,
                        links: Default::default(),
                        array_dim: 1,
                        property_flags: 0,
                        category: 0,
                    })),
                    ..Default::default()
                }));
            }
            let name_index = arena.names.intern(name);
            arena.alloc(UObject {
                name_index,
                outer: Some(root),
                data: ObjectData::ScriptStruct(crate::arena::StructData {
                    children,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        arena
    }

    fn eval(frame: &mut Frame<'_>, name: &str, a: &CallArgs) -> PropValue {
        lookup(name).unwrap_or_else(|| panic!("{name} missing from the pure table"))(frame, a)
            .expect("pure native succeeds")
    }

    #[test]
    fn int_family_reads_both_operands() {
        let arena = ObjectArena::new();
        let mut f = empty_frame(&arena);
        assert_eq!(
            eval(
                &mut f,
                "add_intint",
                &args(&[PropValue::Int(7), PropValue::Int(3)])
            ),
            PropValue::Int(10)
        );
        assert_eq!(
            eval(
                &mut f,
                "subtract_intint",
                &args(&[PropValue::Int(7), PropValue::Int(3)])
            ),
            PropValue::Int(4)
        );
        assert_eq!(
            eval(
                &mut f,
                "multiply_intint",
                &args(&[PropValue::Int(-6), PropValue::Int(2)])
            ),
            PropValue::Int(-12)
        );
        assert_eq!(
            eval(
                &mut f,
                "divide_intint",
                &args(&[PropValue::Int(9), PropValue::Int(2)])
            ),
            PropValue::Int(4)
        );
        assert_eq!(
            eval(
                &mut f,
                "divide_intint",
                &args(&[PropValue::Int(9), PropValue::Int(0)])
            ),
            PropValue::Int(0)
        );
        assert_eq!(
            eval(
                &mut f,
                "less_intint",
                &args(&[PropValue::Int(1), PropValue::Int(2)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "equalequal_intint",
                &args(&[PropValue::Int(2), PropValue::Int(2)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "notequal_intint",
                &args(&[PropValue::Int(2), PropValue::Int(3)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "and_intint",
                &args(&[PropValue::Int(6), PropValue::Int(3)])
            ),
            PropValue::Int(2)
        );
        assert_eq!(
            eval(
                &mut f,
                "or_intint",
                &args(&[PropValue::Int(6), PropValue::Int(3)])
            ),
            PropValue::Int(7)
        );
        assert_eq!(
            eval(
                &mut f,
                "xor_intint",
                &args(&[PropValue::Int(6), PropValue::Int(3)])
            ),
            PropValue::Int(5)
        );
        assert_eq!(
            eval(&mut f, "complement_preint", &args(&[PropValue::Int(0)])),
            PropValue::Int(!0)
        );
        assert_eq!(
            eval(
                &mut f,
                "lessless_intint",
                &args(&[PropValue::Int(1), PropValue::Int(4)])
            ),
            PropValue::Int(16)
        );
        assert_eq!(
            eval(
                &mut f,
                "greatergreater_intint",
                &args(&[PropValue::Int(16), PropValue::Int(4)])
            ),
            PropValue::Int(1)
        );
        assert_eq!(
            eval(&mut f, "subtract_preint", &args(&[PropValue::Int(5)])),
            PropValue::Int(-5)
        );
        assert_eq!(
            eval(
                &mut f,
                "min",
                &args(&[PropValue::Int(3), PropValue::Int(9)])
            ),
            PropValue::Int(3)
        );
        assert_eq!(
            eval(
                &mut f,
                "max",
                &args(&[PropValue::Int(3), PropValue::Int(9)])
            ),
            PropValue::Int(9)
        );
        assert_eq!(
            eval(
                &mut f,
                "clamp",
                &args(&[PropValue::Int(50), PropValue::Int(0), PropValue::Int(10)])
            ),
            PropValue::Int(10)
        );
        // The left operand must survive: swapping operands flips the result.
        assert_ne!(
            eval(
                &mut f,
                "subtract_intint",
                &args(&[PropValue::Int(3), PropValue::Int(7)])
            ),
            eval(
                &mut f,
                "subtract_intint",
                &args(&[PropValue::Int(7), PropValue::Int(3)])
            )
        );
    }

    #[test]
    fn float_and_math_intrinsics() {
        let arena = ObjectArena::new();
        let mut f = empty_frame(&arena);
        let fl = |v: f32| PropValue::Float(v);
        assert_eq!(
            eval(&mut f, "add_floatfloat", &args(&[fl(1.5), fl(2.25)])),
            fl(3.75)
        );
        assert_eq!(
            eval(&mut f, "subtract_floatfloat", &args(&[fl(1.0), fl(4.0)])),
            fl(-3.0)
        );
        assert_eq!(
            eval(&mut f, "multiply_floatfloat", &args(&[fl(2.0), fl(3.0)])),
            fl(6.0)
        );
        assert_eq!(
            eval(&mut f, "divide_floatfloat", &args(&[fl(1.0), fl(0.0)])),
            fl(0.0)
        );
        assert_eq!(
            eval(&mut f, "percent_floatfloat", &args(&[fl(7.5), fl(2.0)])),
            fl(1.5)
        );
        assert_eq!(
            eval(&mut f, "subtract_prefloat", &args(&[fl(2.0)])),
            fl(-2.0)
        );
        assert_eq!(eval(&mut f, "abs", &args(&[fl(-2.5)])), fl(2.5));
        assert_eq!(eval(&mut f, "sqrt", &args(&[fl(9.0)])), fl(3.0));
        assert_eq!(eval(&mut f, "sqrt", &args(&[fl(-9.0)])), fl(0.0));
        assert_eq!(eval(&mut f, "square", &args(&[fl(4.0)])), fl(16.0));
        assert_eq!(eval(&mut f, "fmin", &args(&[fl(1.0), fl(2.0)])), fl(1.0));
        assert_eq!(eval(&mut f, "fmax", &args(&[fl(1.0), fl(2.0)])), fl(2.0));
        assert_eq!(
            eval(&mut f, "fclamp", &args(&[fl(11.0), fl(0.0), fl(10.0)])),
            fl(10.0)
        );
        assert_eq!(
            eval(&mut f, "lerp", &args(&[fl(0.0), fl(10.0), fl(0.25)])),
            fl(2.5)
        );
        assert!(
            (match eval(&mut f, "sin", &args(&[fl(std::f32::consts::FRAC_PI_2)])) {
                PropValue::Float(v) => v,
                _ => panic!("sin returns float"),
            } - 1.0)
                .abs()
                < 1e-6
        );
        assert_eq!(
            eval(
                &mut f,
                "complementequal_floatfloat",
                &args(&[fl(1.00001), fl(1.0)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "complementequal_floatfloat",
                &args(&[fl(1.1), fl(1.0)])
            ),
            PropValue::Bool(false)
        );
        assert_eq!(
            eval(&mut f, "less_floatfloat", &args(&[fl(1.0), fl(2.0)])),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "multiplymultiply_floatfloat",
                &args(&[fl(2.0), fl(3.0)])
            ),
            fl(8.0)
        );
    }

    #[test]
    fn rand_family_is_deterministic_per_frame_seed() {
        let arena = ObjectArena::new();
        // Identical frames produce identical sequences.
        let mut fa = empty_frame(&arena);
        let mut fb = empty_frame(&arena);
        for _ in 0..8 {
            assert_eq!(fa.next_rand_u32(), fb.next_rand_u32());
        }
        // Rand stays inside [0, Max).
        let mut f = empty_frame(&arena);
        for _ in 0..64 {
            let v = match eval(&mut f, "rand", &args(&[PropValue::Int(5)])) {
                PropValue::Int(v) => v,
                other => panic!("rand returns int, got {other:?}"),
            };
            assert!((0..5).contains(&v));
        }
        // FRand lands in [0, 1).
        for _ in 0..16 {
            match eval(&mut f, "frand", &args(&[])) {
                PropValue::Float(v) => assert!((0.0..1.0).contains(&v)),
                other => panic!("frand returns float, got {other:?}"),
            }
        }
    }

    #[test]
    fn bool_name_object_comparisons() {
        let arena = ObjectArena::new();
        let mut f = empty_frame(&arena);
        assert_eq!(
            eval(&mut f, "not_prebool", &args(&[PropValue::Bool(false)])),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "andand_boolbool",
                &args(&[PropValue::Bool(true), PropValue::Bool(false)])
            ),
            PropValue::Bool(false)
        );
        assert_eq!(
            eval(
                &mut f,
                "oror_boolbool",
                &args(&[PropValue::Bool(true), PropValue::Bool(false)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "xorxor_boolbool",
                &args(&[PropValue::Bool(true), PropValue::Bool(true)])
            ),
            PropValue::Bool(false)
        );
        let n = crate::name::Name {
            index: 12,
            number: crate::name::NO_NUMBER,
        };
        let same = n;
        let other = crate::name::Name {
            index: 13,
            number: crate::name::NO_NUMBER,
        };
        assert_eq!(
            eval(
                &mut f,
                "equalequal_namename",
                &args(&[PropValue::Name(n), PropValue::Name(same)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "notequal_namename",
                &args(&[PropValue::Name(n), PropValue::Name(other)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "equalequal_objectobject",
                &args(&[PropValue::Object(Some(3)), PropValue::Object(Some(3))])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "equalequal_objectobject",
                &args(&[PropValue::Object(None), PropValue::Object(Some(3))])
            ),
            PropValue::Bool(false)
        );
        assert_eq!(
            eval(
                &mut f,
                "notequal_objectobject",
                &args(&[PropValue::Object(Some(1)), PropValue::Object(Some(2))])
            ),
            PropValue::Bool(true)
        );
    }

    #[test]
    fn string_builtins() {
        let arena = ObjectArena::new();
        let mut f = empty_frame(&arena);
        let s = |v: &str| PropValue::Str(v.to_string());
        assert_eq!(
            eval(&mut f, "concat_strstr", &args(&[s("Har"), s("ry")])),
            s("Harry")
        );
        assert_eq!(
            eval(
                &mut f,
                "concat_strstr",
                &args(&[s("thread_"), PropValue::Int(7)])
            ),
            s("thread_7"),
            "script numeric-to-string coercion must use the value, not Debug syntax"
        );
        assert_eq!(
            eval(&mut f, "len", &args(&[s("Privet")])),
            PropValue::Int(6)
        );
        assert_eq!(
            eval(&mut f, "instr", &args(&[s("PrivetDr"), s("etD")])),
            PropValue::Int(4)
        );
        assert_eq!(
            eval(&mut f, "instr", &args(&[s("PrivetDr"), s("zzz")])),
            PropValue::Int(-1)
        );
        assert_eq!(
            eval(&mut f, "mid", &args(&[s("PrivetDr"), PropValue::Int(5)])),
            s("tDr")
        );
        assert_eq!(
            eval(
                &mut f,
                "mid",
                &args(&[s("PrivetDr"), PropValue::Int(5), PropValue::Int(2)])
            ),
            s("tD")
        );
        assert_eq!(
            eval(
                &mut f,
                "mid",
                &args(&[s("PrivetDr"), PropValue::Int(3), PropValue::Int(2)])
            ),
            s("ve")
        );
        assert_eq!(
            eval(&mut f, "left", &args(&[s("PrivetDr"), PropValue::Int(6)])),
            s("Privet")
        );
        assert_eq!(
            eval(&mut f, "right", &args(&[s("PrivetDr"), PropValue::Int(2)])),
            s("Dr")
        );
        assert_eq!(eval(&mut f, "caps", &args(&[s("hagrid")])), s("HAGRID"));
        assert_eq!(eval(&mut f, "chr", &args(&[PropValue::Int(65)])), s("A"));
        assert_eq!(eval(&mut f, "asc", &args(&[s("A")])), PropValue::Int(65));
        assert_eq!(
            eval(&mut f, "at_strstr", &args(&[s("Wand"), PropValue::Int(1)])),
            s("a")
        );
        assert_eq!(
            eval(&mut f, "equalequal_strstr", &args(&[s("owl"), s("OWL")])),
            PropValue::Bool(false)
        );
        assert_eq!(
            eval(
                &mut f,
                "complementequal_strstr",
                &args(&[s("owl"), s("OWL")])
            ),
            PropValue::Bool(true)
        );
    }

    #[test]
    fn vector_math_against_core_vector_template() {
        let arena = core_arena();
        let mut f = empty_frame(&arena);
        // Precompute field indices so value constructors don't borrow `f`.
        let vi = |n: &str| arena.names.find_index(n).unwrap();
        let vname = vi("Vector");
        let vec = move |x: f32, y: f32, z: f32| PropValue::Struct {
            struct_name: vname,
            fields: vec![
                (vi("X"), PropValue::Float(x)),
                (vi("Y"), PropValue::Float(y)),
                (vi("Z"), PropValue::Float(z)),
            ],
        };
        let as_vec = |v: &PropValue| match v {
            PropValue::Struct { fields, .. } => fields
                .iter()
                .map(|(_, v)| match v {
                    PropValue::Float(x) => *x,
                    _ => 0.0,
                })
                .collect::<Vec<f32>>(),
            other => panic!("expected struct, got {other:?}"),
        };
        let out = eval(
            &mut f,
            "add_vectorvector",
            &args(&[vec(1.0, 2.0, 3.0), vec(10.0, 20.0, 30.0)]),
        );
        assert_eq!(as_vec(&out), vec![11.0, 22.0, 33.0]);
        let out = eval(
            &mut f,
            "subtract_vectorvector",
            &args(&[vec(10.0, 20.0, 30.0), vec(1.0, 2.0, 3.0)]),
        );
        assert_eq!(as_vec(&out), vec![9.0, 18.0, 27.0]);
        let out = eval(
            &mut f,
            "multiply_vectorfloat",
            &args(&[vec(1.0, -2.0, 3.0), PropValue::Float(2.0)]),
        );
        assert_eq!(as_vec(&out), vec![2.0, -4.0, 6.0]);
        let out = eval(
            &mut f,
            "multiply_floatvector",
            &args(&[PropValue::Float(0.5), vec(2.0, 4.0, 6.0)]),
        );
        assert_eq!(as_vec(&out), vec![1.0, 2.0, 3.0]);
        let out = eval(
            &mut f,
            "divide_vectorfloat",
            &args(&[vec(3.0, 6.0, 9.0), PropValue::Float(3.0)]),
        );
        assert_eq!(as_vec(&out), vec![1.0, 2.0, 3.0]);
        assert_eq!(
            eval(
                &mut f,
                "dot_vectorvector",
                &args(&[vec(1.0, 2.0, 3.0), vec(4.0, 5.0, 6.0)])
            ),
            PropValue::Float(32.0)
        );
        let out = eval(
            &mut f,
            "cross_vectorvector",
            &args(&[vec(1.0, 0.0, 0.0), vec(0.0, 1.0, 0.0)]),
        );
        assert_eq!(as_vec(&out), vec![0.0, 0.0, 1.0]);
        assert_eq!(
            eval(&mut f, "vsize", &args(&[vec(3.0, 4.0, 0.0)])),
            PropValue::Float(5.0)
        );
        let out = eval(&mut f, "normal", &args(&[vec(0.0, 5.0, 0.0)]));
        assert_eq!(as_vec(&out), vec![0.0, 1.0, 0.0]);
        // Mirror of (1,-1,0) across normal (0,1,0) is (1,1,0).
        let out = eval(
            &mut f,
            "mirrorvectorbynormal",
            &args(&[vec(1.0, -1.0, 0.0), vec(0.0, 1.0, 0.0)]),
        );
        assert_eq!(as_vec(&out), vec![1.0, 1.0, 0.0]);
        assert_eq!(
            eval(
                &mut f,
                "equalequal_vectorvector",
                &args(&[vec(1.0, 2.0, 3.0), vec(1.0, 2.0, 3.0)])
            ),
            PropValue::Bool(true)
        );
        // VRand is a unit vector.
        let out = eval(&mut f, "vrand", &args(&[]));
        let c = as_vec(&out);
        assert!((c[0] * c[0] + c[1] * c[1] + c[2] * c[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rotator_math_and_vector_rotation() {
        let arena = core_arena();
        let registry = NativeRegistry::new();
        let mut f = Frame::new(&arena, &registry, &[], None);
        let vi = |n: &str| arena.names.find_index(n).unwrap();
        let (rname, vname) = (vi("Rotator"), vi("Vector"));
        let rot = move |p: i32, y: i32, r: i32| PropValue::Struct {
            struct_name: rname,
            fields: vec![
                (vi("Pitch"), PropValue::Int(p)),
                (vi("Yaw"), PropValue::Int(y)),
                (vi("Roll"), PropValue::Int(r)),
            ],
        };
        let vec = move |x: f32, y: f32, z: f32| PropValue::Struct {
            struct_name: vname,
            fields: vec![
                (vi("X"), PropValue::Float(x)),
                (vi("Y"), PropValue::Float(y)),
                (vi("Z"), PropValue::Float(z)),
            ],
        };
        let as_vec = |v: &PropValue| match v {
            PropValue::Struct { fields, .. } => fields
                .iter()
                .map(|(_, v)| if let PropValue::Float(x) = v { *x } else { 0.0 })
                .collect::<Vec<f32>>(),
            other => panic!("expected struct, got {other:?}"),
        };
        assert_eq!(
            eval(
                &mut f,
                "equalequal_rotatorrotator",
                &args(&[rot(1, 2, 3), rot(1, 2, 3)])
            ),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut f,
                "notequal_rotatorrotator",
                &args(&[rot(1, 2, 3), rot(3, 2, 1)])
            ),
            PropValue::Bool(true)
        );
        let out = eval(
            &mut f,
            "add_rotatorrotator",
            &args(&[rot(100, 200, 300), rot(10, 20, 30)]),
        );
        match out {
            PropValue::Struct { fields, .. } => {
                let vals: Vec<i32> = fields
                    .iter()
                    .map(|(_, v)| if let PropValue::Int(i) = v { *i } else { 0 })
                    .collect();
                assert_eq!(vals, vec![110, 220, 330]);
            }
            other => panic!("expected rotator, got {other:?}"),
        }
        // Yaw 16384 (90°) rotates forward (1,0,0) onto right (0,1,0).
        let out = eval(
            &mut f,
            "greatergreater_vectorrotator",
            &args(&[vec(1.0, 0.0, 0.0), rot(0, 16384, 0)]),
        );
        let c = as_vec(&out);
        assert!(c[0].abs() < 1e-5 && (c[1] - 1.0).abs() < 1e-5 && c[2].abs() < 1e-5);
        // Inverse rotation maps it back.
        let out = eval(
            &mut f,
            "lessless_vectorrotator",
            &args(&[out.clone(), rot(0, 16384, 0)]),
        );
        let c = as_vec(&out);
        assert!((c[0] - 1.0).abs() < 1e-5 && c[1].abs() < 1e-5 && c[2].abs() < 1e-5);
    }

    #[test]
    fn only_unimplemented_compound_families_stay_deferred() {
        for name in ["subtractequal_intint", "divideequal_vectorfloat"] {
            assert!(lookup(name).is_none(), "{name} must stay deferred");
        }
        assert!(lookup("addequal_intint").is_some());
        assert!(lookup("addequal_floatfloat").is_some());
        assert!(lookup("multiplyequal_floatfloat").is_some());
        assert!(lookup("addadd_preint").is_some());
        assert!(lookup("addadd_int").is_some());
        assert!(lookup("subtractsubtract_preint").is_some());
        assert!(lookup("subtractsubtract_int").is_some());
    }

    #[test]
    fn engine_natives_queue_typed_effects() {
        let mut arena = core_arena();
        let root = arena.root_for("Test");
        let actor_name = arena.names.intern("Actor");
        let actor = arena.alloc(UObject {
            name_index: actor_name,
            outer: Some(root),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let cue_name = arena.names.intern("Cue");
        let cue = arena.alloc(UObject {
            name_index: cue_name,
            outer: Some(root),
            ..Default::default()
        });
        let registry = NativeRegistry::new();
        let mut frame = Frame::new(&arena, &registry, &[], Some(actor));
        let location = vector_value(&frame, [12.0, -4.0, 7.5]).expect("vector");

        assert_eq!(
            eval(&mut frame, "setlocation", &args(&[location])),
            PropValue::Bool(true)
        );
        assert_eq!(
            eval(
                &mut frame,
                "playsound",
                &args(&[PropValue::Object(Some(cue.0 as i32))])
            ),
            PropValue::Int(0)
        );
        assert_eq!(
            eval(&mut frame, "stopmusic", &args(&[PropValue::Int(0)])),
            PropValue::Int(0)
        );
        let music_handle = match eval(
            &mut frame,
            "playmusic",
            &args(&[PropValue::Str("PrivetDr".to_string())]),
        ) {
            PropValue::Int(handle) if handle != 0 => handle,
            other => panic!("PlayMusic must return a valid handle, got {other:?}"),
        };
        assert_eq!(
            eval(
                &mut frame,
                "stopmusic",
                &args(&[PropValue::Int(music_handle)])
            ),
            PropValue::Int(0)
        );
        assert_eq!(
            eval(&mut frame, "destroy", &args(&[])),
            PropValue::Bool(true)
        );
        assert_eq!(
            frame.take_effects(),
            vec![
                crate::vm::ScriptEffect::SetLocation {
                    actor,
                    location: [12.0, -4.0, 7.5],
                },
                crate::vm::ScriptEffect::PlaySound { actor, cue },
                crate::vm::ScriptEffect::AudioUnavailable {
                    actor: Some(actor),
                    source: "StopMusic invalid/unstarted handle 0".to_string(),
                },
                crate::vm::ScriptEffect::PlayMusic {
                    actor,
                    song: "PrivetDr".to_string(),
                    handle: music_handle,
                },
                crate::vm::ScriptEffect::StopMusic {
                    actor,
                    handle: music_handle,
                },
                crate::vm::ScriptEffect::Destroy { actor },
            ]
        );
    }

    #[test]
    fn make_noise_queues_sender_and_loudness_without_a_fallback() {
        let mut arena = core_arena();
        let root = arena.root_for("Test");
        let actor_name = arena.names.intern("NoiseMaker");
        let actor = arena.alloc(UObject {
            name_index: actor_name,
            outer: Some(root),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let registry = NativeRegistry::new();
        let mut frame = Frame::new(&arena, &registry, &[], Some(actor));

        assert_eq!(
            eval(
                &mut frame,
                "MakeNoise",
                &args(&[PropValue::Float(3.25)])
            ),
            PropValue::Int(0)
        );
        assert_eq!(
            frame.take_effects(),
            vec![crate::vm::ScriptEffect::MakeNoise {
                actor,
                loudness: 3.25,
            }]
        );

        let mut no_self = empty_frame(&arena);
        let failure = nat_make_noise(&mut no_self, &args(&[PropValue::Float(1.0)]))
            .expect_err("MakeNoise without an actor self must fail loudly");
        assert_eq!(failure.reason_code, "native.actor_self_missing");
        assert!(no_self.take_effects().is_empty());
    }

    #[test]
    fn all_actors_iterator_runs_every_matching_actor_in_allocation_order() {
        let mut arena = ObjectArena::new();
        let out = arena.names.intern("OutActor");
        assert_eq!(
            out, 0,
            "name remains distinct from the raw object reference"
        );
        let root = arena.root_for("Engine");
        let actor_class_name = arena.names.intern("Actor");
        let actor_class = arena.alloc(UObject {
            name_index: actor_class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let first_name = arena.names.intern("First");
        let first = arena.alloc(UObject {
            class_id: Some(actor_class),
            name_index: first_name,
            outer: Some(root),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let second_name = arena.names.intern("Second");
        let second = arena.alloc(UObject {
            class_id: Some(actor_class),
            name_index: second_name,
            outer: Some(root),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let mut registry = NativeRegistry::new();
        registry
            .register(0x70, 0, nat_all_actors, "Engine.Actor.AllActors")
            .expect("register AllActors");
        let code = [
            crate::vm::USToken::Iterator as u8,
            0x70,
            crate::vm::USToken::ObjectConst as u8,
            1, // raw export reference to Actor class
            crate::vm::USToken::LocalVariable as u8,
            2, // raw export reference to OutActor property
            crate::vm::USToken::EndFunctionParms as u8,
            16,
            0,
            crate::vm::USToken::IteratorNext as u8,
            crate::vm::USToken::IteratorPop as u8,
        ];
        let output_property = arena.alloc(UObject {
            name_index: out,
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Object { class_ref: None },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let resolver =
            BytecodeResolver::from_package_refs(Vec::new(), vec![actor_class, output_property]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(first), resolver);
        frame.set_actor_scope(&[first, second]);
        frame.run().expect("foreach AllActors");
        assert_eq!(
            frame.locals.get(out),
            Some(&PropValue::Object(Some(second.0 as i32)))
        );
    }

    #[test]
    fn fast_trace_uses_actor_location_only_when_start_is_omitted() {
        let mut arena = core_arena();
        let engine = arena.root_for("Engine");
        let location = arena.names.intern("Location");
        let vector = arena.names.find_index("Vector").expect("Core.Vector");
        let x = arena.names.find_index("X").expect("Core.Vector.X");
        let y = arena.names.find_index("Y").expect("Core.Vector.Y");
        let z = arena.names.find_index("Z").expect("Core.Vector.Z");
        let source_name = arena.names.intern("TraceSource");
        let vec = |values: [f32; 3]| PropValue::Struct {
            struct_name: vector,
            fields: vec![
                (x, PropValue::Float(values[0])),
                (y, PropValue::Float(values[1])),
                (z, PropValue::Float(values[2])),
            ],
        };
        let actor = arena.alloc(UObject {
            name_index: source_name,
            outer: Some(engine),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        if let ObjectData::Properties(properties) = &mut arena.get_mut(actor).expect("actor").data {
            properties.set(location, vec([10.0, 20.0, 30.0]));
        }
        let registry = NativeRegistry::new();
        let end = vec([100.0, 200.0, 300.0]);
        let mut default_start = Frame::new(&arena, &registry, &[], Some(actor));
        assert_eq!(
            nat_fast_trace(&mut default_start, &args(&[end.clone()])).expect("FastTrace"),
            PropValue::Bool(false)
        );
        assert_eq!(
            default_start.take_effects(),
            vec![crate::vm::ScriptEffect::FastTraceRequest {
                request_id: 0,
                source: actor,
                start: [10.0, 20.0, 30.0],
                end: [100.0, 200.0, 300.0],
            }]
        );

        let mut explicit_start = Frame::new(&arena, &registry, &[], Some(actor));
        nat_fast_trace(
            &mut explicit_start,
            &args(&[end, vec([-1.0, -2.0, -3.0])]),
        )
        .expect("FastTrace");
        let effects = explicit_start.take_effects();
        let [crate::vm::ScriptEffect::FastTraceRequest { start, .. }] = effects.as_slice() else {
            panic!("unexpected FastTrace effects {effects:?}");
        };
        assert_eq!(*start, [-1.0, -2.0, -3.0]);
    }

    #[test]
    fn actor_error_coerces_its_message_and_has_no_effects() {
        let arena = ObjectArena::new();
        let mut frame = empty_frame(&arena);
        let fail = nat_actor_error(&mut frame, &args(&[PropValue::Int(233)]))
            .expect_err("Actor.Error must fail");
        assert_eq!(fail.reason_code, "engine.actor_error");
        assert_eq!(fail.message, "233");
        assert!(frame.take_effects().is_empty());
    }

    #[test]
    fn log_writes_to_stderr_and_succeeds() {
        let arena = ObjectArena::new();
        let mut f = empty_frame(&arena);
        let out = eval(
            &mut f,
            "log",
            &args(&[PropValue::Str("census marker".to_string())]),
        );
        assert_eq!(out, PropValue::Int(0));
        assert_eq!(
            f.take_effects(),
            vec![crate::vm::ScriptEffect::Log {
                message: "census marker".to_string(),
            }]
        );
        for index in 0..257 {
            f.log_effect(format!("bounded-{index}")).expect("bounded log effect");
        }
        let effects = f.take_effects();
        assert_eq!(effects.len(), 256);
        assert_eq!(
            effects.first(),
            Some(&crate::vm::ScriptEffect::Log {
                message: "bounded-0".to_string(),
            })
        );
        assert_eq!(eval(&mut f, "saveconfig", &args(&[])), PropValue::Int(0));
    }
}
