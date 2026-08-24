//! Byte-exact readers for the UE1 packed mesh structures
//! (`FMeshVert`, `FMeshFace`, `FMeshVertConnect`) as serialized by the
//! first-party engine (little-endian arm64 build).

/// Packed 11/11/10-bit signed vertex, one little-endian `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshVert {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

fn sign_extend(value: u32, bits: u32) -> i32 {
    let sign_bit = 1u32 << (bits - 1);
    if value & sign_bit != 0 {
        value as i32 - (1i32 << bits)
    } else {
        value as i32
    }
}

impl MeshVert {
    pub const SIZE_BYTES: usize = 4;

    /// Decode one packed vertex from a little-endian slice.
    pub fn from_le(bytes: &[u8]) -> Option<MeshVert> {
        let raw = <[u8; 4]>::try_from(bytes.get(0..4)?).ok()?;
        let packed = u32::from_le_bytes(raw);
        Some(MeshVert::from_packed(packed))
    }

    pub fn from_packed(packed: u32) -> MeshVert {
        MeshVert {
            x: sign_extend(packed & 0x7ff, 11),
            y: sign_extend((packed >> 11) & 0x7ff, 11),
            z: sign_extend((packed >> 22) & 0x3ff, 10),
        }
    }

    pub fn to_packed(self) -> u32 {
        (self.x as u32 & 0x7ff)
            | ((self.y as u32 & 0x7ff) << 11)
            | ((self.z as u32 & 0x3ff) << 22)
    }
}

/// Triangle referencing three wedge indices plus its material slot —
/// exactly 8 bytes on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshFace {
    pub i_wedge: [u16; 3],
    pub material_index: u16,
}

impl MeshFace {
    pub const SIZE_BYTES: usize = 8;

    pub fn from_le(bytes: &[u8]) -> Option<MeshFace> {
        let raw = <[u8; 8]>::try_from(bytes.get(0..8)?).ok()?;
        Some(MeshFace {
            i_wedge: [
                u16::from_le_bytes([raw[0], raw[1]]),
                u16::from_le_bytes([raw[2], raw[3]]),
                u16::from_le_bytes([raw[4], raw[5]]),
            ],
            material_index: u16::from_le_bytes([raw[6], raw[7]]),
        })
    }

    pub fn to_le_bytes(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        for (index, wedge) in self.i_wedge.iter().enumerate() {
            out[index * 2..index * 2 + 2].copy_from_slice(&wedge.to_le_bytes());
        }
        out[6..8].copy_from_slice(&self.material_index.to_le_bytes());
        out
    }
}

/// Vertex-to-triangle adjacency entry — two little-endian `i32`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshVertConnect {
    pub num_vert_triangles: i32,
    pub triangle_list_offset: i32,
}

impl MeshVertConnect {
    pub const SIZE_BYTES: usize = 8;

    pub fn from_le(bytes: &[u8]) -> Option<MeshVertConnect> {
        let raw = <[u8; 8]>::try_from(bytes.get(0..8)?).ok()?;
        Some(MeshVertConnect {
            num_vert_triangles: i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]),
            triangle_list_offset: i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        })
    }
}
