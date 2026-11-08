//! ULodMesh archive order and packed animation remapping. Native code owns all
//! typed arrays; the only destination allocation is the eventual Verts buffer.
use std::ffi::c_void;

/// cbindgen:prefix-with-name=true
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2LodArchiveAction {
    Saving,
    EmptyTris,
    Super,
    CollapsePointThus,
    FaceLevel,
    Faces,
    CollapseWedgeThus,
    Wedges,
    Materials,
    SpecialFaces,
    ModelVerts,
    SpecialVerts,
    MeshScaleMax,
    LODHysteresis,
    LODStrength,
    LODMinVerts,
    LODMorph,
    LODZDisplace,
    RemapAnimVerts,
    OldFrameVerts,
    LoadingState,
    LoadVertices,
    AllocateVertices,
    EmptyVertices,
    ExchangeVertices,
    ReleaseVertices,
    ReleaseRemap,
    InvalidDimensions,
    InvalidRemapIndex,
    InvalidVertexIndex,
}

#[repr(C)]
pub struct Hp2LodArchiveState {
    pub saving: i32,
    pub loading: i32,
    pub frame_verts: i32,
    pub anim_frames: i32,
    pub old_frame_verts: i32,
    pub remap_count: i32,
    pub vertex_count: i32,
    pub lod_min_verts: *mut i32,
    pub remap: *const u16,
    pub vertices: *const u8,
    pub destination: *mut u8,
}
pub type Hp2LodArchiveLeaf =
    unsafe extern "C" fn(*mut c_void, Hp2LodArchiveAction, i32, *mut Hp2LodArchiveState) -> i32;

/// Gathers whole FMeshVert object representations (three bitfields occupying
/// four bytes). No float conversion, allocation, or native call per vertex.
///
/// # Safety
/// The state spans must be live, disjoint from the destination, and have the
/// advertised lengths. Destination holds frame_verts * anim_frames elements.
unsafe fn gather(s: &Hp2LodArchiveState) -> Result<(), (Hp2LodArchiveAction, i32)> {
    use Hp2LodArchiveAction::*;
    for frame in 0..s.anim_frames {
        let source_start = frame
            .checked_mul(s.old_frame_verts)
            .ok_or((InvalidDimensions, 0))?;
        let destination_start = frame * s.frame_verts;
        for vertex in 0..s.frame_verts {
            if vertex >= s.remap_count {
                return Err((InvalidRemapIndex, vertex));
            }
            let index = unsafe { *s.remap.add(vertex as usize) } as i32;
            let source = source_start
                .checked_add(index)
                .ok_or((InvalidDimensions, 0))?;
            if source < 0 || source >= s.vertex_count {
                return Err((InvalidVertexIndex, source));
            }
            // Byte alignment is sufficient even on a differently packed SDK.
            let packed = unsafe { s.vertices.cast::<[u8; 4]>().add(source as usize).read() };
            unsafe {
                s.destination
                    .cast::<[u8; 4]>()
                    .add((destination_start + vertex) as usize)
                    .write(packed);
            }
        }
    }
    Ok(())
}

/// Runs the complete ordered Serialize policy. A failing native leaf stops the
/// sequence immediately; C++ retains and rethrows its original exception after
/// this call, and destroys the temporary typed array on its own stack.
///
/// # Safety
/// `context` and `leaf` must provide the live archive/mesh operations described
/// by the action enum. LoadingState provides writable LODMinVerts; LoadVertices
/// borrows source/remap storage until EmptyVertices, after gathering has ended.
pub unsafe fn serialize(context: *mut c_void, leaf: Hp2LodArchiveLeaf) -> i32 {
    use Hp2LodArchiveAction::*;
    let mut s: Hp2LodArchiveState = unsafe { std::mem::zeroed() };
    macro_rules! call {
        ($action:expr, $arg:expr) => {{
            let status = unsafe { leaf(context, $action, $arg, &mut s) };
            if status != 0 {
                return status;
            }
        }};
        ($action:expr) => {
            call!($action, 0)
        };
    }
    call!(Saving);
    if s.saving != 0 {
        call!(EmptyTris);
    }
    for action in [
        Super,
        CollapsePointThus,
        FaceLevel,
        Faces,
        CollapseWedgeThus,
        Wedges,
        Materials,
        SpecialFaces,
        ModelVerts,
        SpecialVerts,
        MeshScaleMax,
        LODHysteresis,
        LODStrength,
        LODMinVerts,
        LODMorph,
        LODZDisplace,
        RemapAnimVerts,
        OldFrameVerts,
    ] {
        call!(action);
    }
    call!(LoadingState);
    if s.loading == 0 {
        return 0;
    }
    unsafe {
        *s.lod_min_verts = (*s.lod_min_verts).min(s.frame_verts);
    }
    if s.remap_count == 0 {
        return 0;
    }
    call!(LoadVertices);
    let count = match s.frame_verts.checked_mul(s.anim_frames) {
        Some(count) => count,
        None => {
            call!(InvalidDimensions);
            return 1;
        }
    };
    // Negative dimensions with positive products would publish uninitialized
    // C++ storage; signed overflow and one-past-end reads are likewise outside
    // native defined behavior, not cases to truncate or silently accept.
    if count > 0 && (s.frame_verts < 0 || s.anim_frames < 0) {
        call!(InvalidDimensions);
        return 1;
    }
    call!(AllocateVertices, count);
    // Let native TArray::Add report a negative count with its original check.
    // If that check was compiled out, never continue into unchecked storage.
    if count < 0 {
        call!(InvalidDimensions);
        return 1;
    }
    if let Err((action, index)) = unsafe { gather(&s) } {
        call!(action, index);
        return 1;
    }
    call!(EmptyVertices);
    call!(ExchangeVertices);
    call!(ReleaseVertices);
    call!(ReleaseRemap);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(
        source: &[[u8; 4]],
        remap: &[u16],
        output: &mut [[u8; 4]],
        frames: i32,
        old: i32,
    ) -> Hp2LodArchiveState {
        Hp2LodArchiveState {
            saving: 0,
            loading: 1,
            frame_verts: remap.len() as i32,
            anim_frames: frames,
            old_frame_verts: old,
            remap_count: remap.len() as i32,
            vertex_count: source.len() as i32,
            lod_min_verts: std::ptr::null_mut(),
            remap: remap.as_ptr(),
            vertices: source.as_ptr().cast(),
            destination: output.as_mut_ptr().cast(),
        }
    }

    #[test]
    fn duplicate_discard_and_stride_preserve_packed_bits() {
        let source = [
            [0xff; 4],
            [0; 4],
            [0, 0xf8, 0x3f, 0x80],
            [1; 4],
            [2; 4],
            [3; 4],
        ];
        let mut output = [[0; 4]; 8];
        let s = state(&source, &[2, 0, 2, 0], &mut output, 2, 3);
        assert!(unsafe { gather(&s) }.is_ok());
        assert_eq!(
            output,
            [
                source[2], source[0], source[2], source[0], source[5], source[3], source[5],
                source[3]
            ]
        );
    }

    #[test]
    fn zero_frames_never_dereference_remap_or_source() {
        let mut s = state(&[], &[], &mut [], 0, i32::MAX);
        s.frame_verts = 12;
        assert!(unsafe { gather(&s) }.is_ok());
    }

    #[test]
    fn invalid_source_stops_before_writing_that_vertex() {
        let mut output = [[0xaa; 4]; 2];
        let s = state(&[[0xff; 4]], &[0, 1], &mut output, 1, 1);
        assert!(matches!(
            unsafe { gather(&s) },
            Err((Hp2LodArchiveAction::InvalidVertexIndex, 1))
        ));
        assert_eq!(output, [[0xff; 4], [0xaa; 4]]);
    }
}
