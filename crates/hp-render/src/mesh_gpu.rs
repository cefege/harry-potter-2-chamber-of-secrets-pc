//! GPU mesh construction from the `hp_format::mesh` byte-exact readers
//! (Milestone M3 foundation).
//!
//! One vertex layout feeds every surface pipeline: position, two UV sets
//! (base + lightmap/detail), per-vertex gouraud color, and a per-vertex fog
//! factor in `[0,1]` (0 = fully fogged toward the pass fog color).

use crate::device::GpuContext;
use hp_format::mesh::{MeshFace, MeshVert};

/// 48 bytes, tightly packed, matching [`crate::pipelines`] vertex layouts.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    /// UV set 0: base texture.
    pub uv0: [f32; 2],
    /// UV set 1: lightmap / detail texture.
    pub uv1: [f32; 2],
    /// RGB = gouraud tint, A = base alpha (masked/translucent passes).
    pub color: [f32; 4],
    /// 0 = fully fogged, 1 = unfogged.
    pub unfogged: f32,
}

impl MeshVertex {
    pub const STRIDE: u64 = std::mem::size_of::<MeshVertex>() as u64;

    pub const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x4,
        4 => Float32,
    ];

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: Self::STRIDE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// UE1 packed mesh vertices are stored scaled by 8 (`FMeshVert`
    /// fixed-point convention observed in the format readers).
    pub const MESH_VERT_SCALE: f32 = 1.0 / 8.0;

    /// Expand one packed reader vertex into a shaded GPU vertex.
    pub fn from_mesh_vert(vert: MeshVert) -> MeshVertex {
        let s = Self::MESH_VERT_SCALE;
        MeshVertex {
            position: [vert.x as f32 * s, vert.y as f32 * s, vert.z as f32 * s],
            uv0: [0.0; 2],
            uv1: [0.0; 2],
            color: [1.0; 4],
            unfogged: 1.0,
        }
    }
}

/// Vertex+index buffers ready to bind in a render pass.
pub struct MeshGpu {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

/// Upload POD data verbatim into a vertex-or-index buffer. Payloads here are
/// `repr(C)` plain-old-data (all-`f32`/`u16`, no padding), so a byte-level
/// copy is sound without pulling in a bytemuck-style dependency.
fn create_buffer_from_pod<T: Copy + 'static>(
    ctx: &GpuContext,
    label: &str,
    usage: wgpu::BufferUsages,
    data: &[T],
) -> wgpu::Buffer {
    let byte_len = std::mem::size_of_val(data);
    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), byte_len) };
    // COPY_BUFFER_ALIGNMENT applies to queue writes too; pad tiny payloads
    // (e.g. a 3-index triangle list) up to 4 bytes.
    let padded_len = byte_len.div_ceil(wgpu::COPY_BUFFER_ALIGNMENT as usize)
        * wgpu::COPY_BUFFER_ALIGNMENT as usize;
    let mut padded = vec![0u8; padded_len];
    padded[..byte_len].copy_from_slice(bytes);
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: padded_len as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&buffer, 0, &padded);
    buffer
}

/// Build GPU buffers from packed mesh readers plus parallel per-wedge UV,
/// color, and fog tables (one entry per wedge index).
///
/// `faces[i].i_wedge[j]` indexes into `verts`; UVs/colors/fog ride the same
/// wedge indices, mirroring how the wire format stores wedge-local
/// attributes. Material selection happens at draw-list level via
/// `MeshFace::material_index`.
pub fn build_mesh(
    ctx: &GpuContext,
    label: &str,
    verts: &[MeshVert],
    faces: &[MeshFace],
    wedge_u0: &[[f32; 2]],
    colors: &[[f32; 4]],
    unfogged: &[f32],
) -> MeshGpu {
    let mut vertices: Vec<MeshVertex> = verts
        .iter()
        .map(|v| MeshVertex::from_mesh_vert(*v))
        .collect();
    for face in faces {
        for &wedge in &face.i_wedge {
            let slot = wedge as usize;
            let Some(vertex) = vertices.get_mut(slot) else {
                continue;
            };
            if let Some(uv) = wedge_u0.get(slot) {
                vertex.uv0 = *uv;
            }
            if let Some(c) = colors.get(slot) {
                vertex.color = *c;
            }
            if let Some(f) = unfogged.get(slot) {
                vertex.unfogged = *f;
            }
        }
    }
    // Front faces are CLOCKWISE in screen space under the left-handed
    // (right, up, forward) view basis — same convention as PipelineSet.
    // Wire faces are authored CCW-front, so expand reversed.
    let mut indices: Vec<u16> = Vec::with_capacity(faces.len() * 3);
    for face in faces {
        indices.extend_from_slice(&[face.i_wedge[0], face.i_wedge[2], face.i_wedge[1]]);
    }

    let vertex_label = format!("{label} vertices");
    let index_label = format!("{label} indices");
    let vertex_buffer =
        create_buffer_from_pod(ctx, &vertex_label, wgpu::BufferUsages::VERTEX, &vertices);
    let index_buffer =
        create_buffer_from_pod(ctx, &index_label, wgpu::BufferUsages::INDEX, &indices);
    MeshGpu {
        vertex_buffer,
        index_buffer,
        num_indices: indices.len() as u32,
    }
}

/// Build GPU buffers from already-expanded [`MeshVertex`]s and a u16 index
/// list. Additive entry point for callers that author vertices directly
/// (e.g. the engine's brush-poly path) without going through the packed
/// `hp_format::mesh` readers.
pub fn build_mesh_manual(
    ctx: &GpuContext,
    label: &str,
    vertices: &[MeshVertex],
    indices: &[u16],
) -> MeshGpu {
    let vertex_label = format!("{label} vertices");
    let index_label = format!("{label} indices");
    let vertex_buffer =
        create_buffer_from_pod(ctx, &vertex_label, wgpu::BufferUsages::VERTEX, vertices);
    let index_buffer =
        create_buffer_from_pod(ctx, &index_label, wgpu::BufferUsages::INDEX, indices);
    MeshGpu {
        vertex_buffer,
        index_buffer,
        num_indices: indices.len() as u32,
    }
}

/// Camera-facing quad (sprite/particle billboard): four vertices plus two
/// triangles in strip order. `size` is (width, height) in world units.
pub fn billboard_quad(
    center: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    size: [f32; 2],
    uv_rect: [[f32; 2]; 4],
    color: [f32; 4],
    unfogged: f32,
) -> ([MeshVertex; 4], [u16; 6]) {
    let half_w = size[0] * 0.5;
    let half_h = size[1] * 0.5;
    let corner = |sr: f32, su: f32, uv: [f32; 2]| MeshVertex {
        position: [
            center[0] + right[0] * sr * half_w + up[0] * su * half_h,
            center[1] + right[1] * sr * half_w + up[1] * su * half_h,
            center[2] + right[2] * sr * half_w + up[2] * su * half_h,
        ],
        uv0: uv,
        uv1: [0.0; 2],
        color,
        unfogged,
    };
    let corners = [
        corner(-1.0, -1.0, uv_rect[0]),
        corner(1.0, -1.0, uv_rect[1]),
        corner(1.0, 1.0, uv_rect[2]),
        corner(-1.0, 1.0, uv_rect[3]),
    ];
    // Front = CW under the left-handed screen basis (see PipelineSet).
    (corners, [0, 2, 1, 0, 3, 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp_format::mesh::{MeshFace, MeshVert};

    #[test]
    fn packed_vertices_expand_with_eighth_scale() {
        let packed = MeshVert::from_packed((8) | (16 << 11) | (((-24i32 as u32) & 0x3FF) << 22));
        let vertex = MeshVertex::from_mesh_vert(packed);
        assert_eq!(packed.x, 8);
        assert_eq!(packed.y, 16);
        assert_eq!(packed.z, -24);
        assert_eq!(vertex.position[0], 1.0);
        assert_eq!(vertex.position[1], 2.0);
        assert_eq!(vertex.position[2], -3.0);
        assert_eq!(MeshVertex::STRIDE, 48);
    }

    #[test]
    fn faces_expand_into_straight_triangle_lists() {
        let ctx = GpuContext::headless().expect("headless Metal device");
        let verts = [MeshVert::from_packed(0); 3];
        let faces = [MeshFace {
            i_wedge: [0, 1, 2],
            material_index: 5,
        }];
        let mesh = build_mesh(&ctx, "tri", &verts, &faces, &[], &[], &[]);
        assert_eq!(mesh.num_indices, 3);
    }

    #[test]
    fn billboards_face_camera_with_uv_corners() {
        let (verts, idx) = billboard_quad(
            [10.0, 20.0, 30.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [4.0, 2.0],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            [1.0, 1.0, 1.0, 0.5],
            1.0,
        );
        assert_eq!(idx, [0, 2, 1, 0, 3, 2]);
        assert_eq!(verts[0].position, [8.0, 19.0, 30.0]);
        assert_eq!(verts[1].position, [12.0, 19.0, 30.0]);
        assert_eq!(verts[2].position, [12.0, 21.0, 30.0]);
        assert_eq!(verts[3].position, [8.0, 21.0, 30.0]);
        assert_eq!(verts[0].color[3], 0.5);
    }
}
