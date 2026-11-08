//! Rust dispatch policy for `URender::Exec`.
//!
//! Native callbacks retain the renderer fields, `FOutputDevice`, and the UE1
//! scalar parsers. Rust owns the ordered command grammar and every branch
//! selection, preserving the legacy `ParseCommand` cursor semantics.

use std::ffi::c_void;
use std::ptr::null;

use hp2_core::ansi::Tchar as TChar;
use hp2_core::command::parse_command;

/// cbindgen:prefix-with-name
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Hp2RenderConsoleLeaf {
    StatNet = 0,
    StatFps = 1,
    StatGlobal = 2,
    StatMesh = 3,
    StatParticle = 4,
    StatActor = 5,
    StatFilter = 6,
    StatReject = 7,
    StatSpan = 8,
    StatZone = 9,
    StatLight = 10,
    StatOcclusion = 11,
    StatGame = 12,
    StatSoft = 13,
    StatCache = 14,
    StatPolyV = 15,
    StatPolyC = 16,
    StatIllum = 17,
    StatHardware = 18,
    StatEari = 19,
    StatEariDetail = 20,
    StatAnim = 21,
    StatMissingAverage = 22,
    StatSetAverage = 23,
    RendLeak = 24,
    RendWire = 25,
    RendBone = 26,
    RendBlend = 27,
    RendToggle = 28,
    RendRecognized = 29,
    Tlod = 30,
    Mlod = 31,
    Mlmode = 32,
    Mlfix = 33,
}

#[repr(C)]
pub struct Hp2RenderConsoleServices {
    pub opaque: *mut c_void,
    /// Return zero on success, otherwise the captured native failure status.
    /// `text` is the command tail for scalar commands and null for toggles.
    pub invoke: unsafe extern "C" fn(*mut c_void, u32, *const c_void) -> i32,
}

impl Hp2RenderConsoleServices {
    unsafe fn call(&self, leaf: Hp2RenderConsoleLeaf, text: *const TChar) -> Result<(), i32> {
        // SAFETY: callers supply a live callback/context and `text` is either
        // null or a cursor into the live terminated command buffer.
        match unsafe { (self.invoke)(self.opaque, leaf as u32, text.cast()) } {
            0 => Ok(()),
            status => Err(status),
        }
    }
}

const fn wide<const N: usize>(bytes: &[u8; N]) -> [TChar; N] {
    let mut result = [0; N];
    let mut index = 0;
    while index < N {
        result[index] = bytes[index] as TChar;
        index += 1;
    }
    result
}

const STAT: [TChar; 5] = wide(b"STAT\0");
const AVERAGE: [TChar; 8] = wide(b"Average\0");
const REND: [TChar; 5] = wide(b"REND\0");
const LEAK: [TChar; 5] = wide(b"LEAK\0");
const WIRE: [TChar; 5] = wide(b"WIRE\0");
const BONE: [TChar; 5] = wide(b"BONE\0");
const BLEND: [TChar; 6] = wide(b"BLEND\0");
const T: [TChar; 2] = wide(b"T\0");
const TLOD: [TChar; 5] = wide(b"TLOD\0");
const MLOD: [TChar; 5] = wide(b"MLOD\0");
const MLMODE: [TChar; 7] = wide(b"MLMODE\0");
const MLFIX: [TChar; 6] = wide(b"MLFIX\0");

const STAT_NET: [TChar; 4] = wide(b"Net\0");
const STAT_FPS: [TChar; 4] = wide(b"Fps\0");
const STAT_GLOBAL: [TChar; 7] = wide(b"Global\0");
const STAT_MESH: [TChar; 5] = wide(b"Mesh\0");
const STAT_PARTICLE: [TChar; 9] = wide(b"Particle\0");
const STAT_ACTOR: [TChar; 6] = wide(b"Actor\0");
const STAT_FILTER: [TChar; 7] = wide(b"Filter\0");
const STAT_REJECT: [TChar; 7] = wide(b"Reject\0");
const STAT_SPAN: [TChar; 5] = wide(b"Span\0");
const STAT_ZONE: [TChar; 5] = wide(b"Zone\0");
const STAT_LIGHT: [TChar; 6] = wide(b"Light\0");
const STAT_OCCLUSION: [TChar; 10] = wide(b"Occlusion\0");
const STAT_GAME: [TChar; 5] = wide(b"Game\0");
const STAT_SOFT: [TChar; 5] = wide(b"Soft\0");
const STAT_CACHE: [TChar; 6] = wide(b"Cache\0");
const STAT_POLY_V: [TChar; 6] = wide(b"PolyV\0");
const STAT_POLY_C: [TChar; 6] = wide(b"PolyC\0");
const STAT_ILLUM: [TChar; 6] = wide(b"Illum\0");
const STAT_HARDWARE: [TChar; 9] = wide(b"Hardware\0");
const STAT_EARI: [TChar; 5] = wide(b"EARI\0");
const STAT_EARI_DETAIL: [TChar; 11] = wide(b"EARIDetail\0");
const STAT_ANIM: [TChar; 5] = wide(b"Anim\0");

const STAT_TOGGLES: [(&[TChar], Hp2RenderConsoleLeaf); 22] = [
    (&STAT_NET, Hp2RenderConsoleLeaf::StatNet),
    (&STAT_FPS, Hp2RenderConsoleLeaf::StatFps),
    (&STAT_GLOBAL, Hp2RenderConsoleLeaf::StatGlobal),
    (&STAT_MESH, Hp2RenderConsoleLeaf::StatMesh),
    (&STAT_PARTICLE, Hp2RenderConsoleLeaf::StatParticle),
    (&STAT_ACTOR, Hp2RenderConsoleLeaf::StatActor),
    (&STAT_FILTER, Hp2RenderConsoleLeaf::StatFilter),
    (&STAT_REJECT, Hp2RenderConsoleLeaf::StatReject),
    (&STAT_SPAN, Hp2RenderConsoleLeaf::StatSpan),
    (&STAT_ZONE, Hp2RenderConsoleLeaf::StatZone),
    (&STAT_LIGHT, Hp2RenderConsoleLeaf::StatLight),
    (&STAT_OCCLUSION, Hp2RenderConsoleLeaf::StatOcclusion),
    (&STAT_GAME, Hp2RenderConsoleLeaf::StatGame),
    (&STAT_SOFT, Hp2RenderConsoleLeaf::StatSoft),
    (&STAT_CACHE, Hp2RenderConsoleLeaf::StatCache),
    (&STAT_POLY_V, Hp2RenderConsoleLeaf::StatPolyV),
    (&STAT_POLY_C, Hp2RenderConsoleLeaf::StatPolyC),
    (&STAT_ILLUM, Hp2RenderConsoleLeaf::StatIllum),
    (&STAT_HARDWARE, Hp2RenderConsoleLeaf::StatHardware),
    (&STAT_EARI, Hp2RenderConsoleLeaf::StatEari),
    (&STAT_EARI_DETAIL, Hp2RenderConsoleLeaf::StatEariDetail),
    (&STAT_ANIM, Hp2RenderConsoleLeaf::StatAnim),
];

/// Runs the legacy command chain through the Native adapter.
///
/// # Safety
/// The command buffer remains live and NUL-terminated throughout the synchronous dispatch.
pub unsafe fn exec(
    services: &Hp2RenderConsoleServices,
    command: *const TChar,
) -> Result<bool, i32> {
    unsafe { exec_with(command, |leaf, text| services.call(leaf, text)) }
}

/// Runs the shared command grammar with a typed host for every renderer operation.
///
/// # Safety
/// A non-null `command` is a live NUL-terminated TCHAR string. `invoke` must not retain
/// `text`, which is null or a cursor into that buffer.
pub unsafe fn exec_with<E>(
    command: *const TChar,
    mut invoke: impl FnMut(Hp2RenderConsoleLeaf, *const TChar) -> Result<(), E>,
) -> Result<bool, E> {
    if command.is_null() {
        return Ok(false);
    }
    let mut cursor = command;
    // SAFETY: the command is a live terminated TCHAR buffer and every literal
    // is compile-time terminated. Calls preserve its storage.
    unsafe {
        if parse_command(&mut cursor, STAT.as_ptr()) != 0 {
            for (token, leaf) in STAT_TOGGLES {
                if parse_command(&mut cursor, token.as_ptr()) != 0 {
                    invoke(leaf, null())?;
                }
            }
            if parse_command(&mut cursor, AVERAGE.as_ptr()) != 0 {
                if *cursor == 0 {
                    invoke(Hp2RenderConsoleLeaf::StatMissingAverage, null())?;
                } else {
                    invoke(Hp2RenderConsoleLeaf::StatSetAverage, cursor)?;
                }
            }
            return Ok(true);
        }
        if parse_command(&mut cursor, REND.as_ptr()) != 0 {
            let leaf = if parse_command(&mut cursor, LEAK.as_ptr()) != 0 {
                Hp2RenderConsoleLeaf::RendLeak
            } else if parse_command(&mut cursor, WIRE.as_ptr()) != 0 {
                Hp2RenderConsoleLeaf::RendWire
            } else if parse_command(&mut cursor, BONE.as_ptr()) != 0 {
                Hp2RenderConsoleLeaf::RendBone
            } else if parse_command(&mut cursor, BLEND.as_ptr()) != 0 {
                Hp2RenderConsoleLeaf::RendBlend
            } else if parse_command(&mut cursor, T.as_ptr()) != 0 {
                Hp2RenderConsoleLeaf::RendToggle
            } else {
                invoke(Hp2RenderConsoleLeaf::RendRecognized, null())?;
                return Ok(false);
            };
            invoke(leaf, null())?;
            return Ok(true);
        }
        for (token, leaf) in [
            (TLOD.as_ptr(), Hp2RenderConsoleLeaf::Tlod),
            (MLOD.as_ptr(), Hp2RenderConsoleLeaf::Mlod),
            (MLMODE.as_ptr(), Hp2RenderConsoleLeaf::Mlmode),
            (MLFIX.as_ptr(), Hp2RenderConsoleLeaf::Mlfix),
        ] {
            if parse_command(&mut cursor, token) != 0 {
                invoke(leaf, cursor)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recording {
        calls: Vec<(Hp2RenderConsoleLeaf, Option<Vec<TChar>>)>,
    }

    unsafe extern "C" fn invoke(opaque: *mut c_void, op: u32, text: *const c_void) -> i32 {
        let recording = unsafe { &mut *opaque.cast::<Recording>() };
        let tail = text.cast::<TChar>();
        let tail = if tail.is_null() {
            None
        } else {
            let mut length = 0;
            while unsafe { *tail.add(length) } != 0 {
                length += 1;
            }
            Some(unsafe { std::slice::from_raw_parts(tail, length) }.to_vec())
        };
        let leaf = unsafe { std::mem::transmute(op) };
        recording.calls.push((leaf, tail));
        0
    }

    fn run(command: &str) -> (bool, Recording) {
        let mut command: Vec<TChar> = command.bytes().map(TChar::from).collect();
        command.push(0);
        let mut recording = Recording::default();
        let services = Hp2RenderConsoleServices {
            opaque: (&mut recording as *mut Recording).cast(),
            invoke,
        };
        let handled = unsafe { exec(&services, command.as_ptr()) }.unwrap();
        (handled, recording)
    }

    #[test]
    fn stat_chain_preserves_toggle_order_and_average_tail() {
        let (handled, recording) = run(" \tStAt Net FpS Average 12.5 ignored");
        assert!(handled);
        assert_eq!(
            recording.calls,
            vec![
                (Hp2RenderConsoleLeaf::StatNet, None),
                (Hp2RenderConsoleLeaf::StatFps, None),
                (
                    Hp2RenderConsoleLeaf::StatSetAverage,
                    Some("12.5 ignored".bytes().map(TChar::from).collect()),
                ),
            ]
        );
    }

    #[test]
    fn rend_unknown_logs_but_is_unhandled() {
        let (handled, recording) = run("REND unknown");
        assert!(!handled);
        assert_eq!(
            recording.calls,
            vec![(Hp2RenderConsoleLeaf::RendRecognized, None)]
        );
    }

    #[test]
    fn scalar_commands_forward_the_original_parsed_tail() {
        let (handled, recording) = run(" MLFIX \t0.75 extra");
        assert!(handled);
        assert_eq!(
            recording.calls,
            vec![(
                Hp2RenderConsoleLeaf::Mlfix,
                Some("0.75 extra".bytes().map(TChar::from).collect()),
            )]
        );
    }
}
