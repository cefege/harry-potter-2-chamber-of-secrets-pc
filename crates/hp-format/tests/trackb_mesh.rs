//! Byte-layout tests for the packed mesh structs. Boundary values pin the
//! sign-extension and field-order behavior of the C++ serializers.

use hp_format::mesh::{MeshFace, MeshVert, MeshVertConnect};

#[test]
fn mesh_vert_positive_and_negative_extremes() {
    // X=-1024 (11-bit min), Y=1023 (11-bit max), Z=-512 (10-bit min).
    let packed = MeshVert { x: -1024, y: 1023, z: -512 }.to_packed();
    assert_eq!(packed, (512u32 << 22) | (1023u32 << 11) | 0x400);
    let decoded = MeshVert::from_packed(packed);
    assert_eq!(decoded, MeshVert { x: -1024, y: 1023, z: -512 });
}

#[test]
fn mesh_vert_zero_round_trip() {
    let zero = MeshVert { x: 0, y: 0, z: 0 };
    assert_eq!(MeshVert::from_packed(zero.to_packed()), zero);
    assert_eq!(zero.to_packed(), 0);
}

#[test]
fn mesh_vert_wire_form_is_four_little_endian_bytes() {
    // X=1,Y=2,Z=3 -> bytes 0xB0? verify exact layout: packed =
    //   x(1) | y(2)<<11 | z(3)<<22 = 0x00C_1001
    let vert = MeshVert { x: 1, y: 2, z: 3 };
    let mut bytes = vec![];
    bytes.extend_from_slice(&vert.to_packed().to_le_bytes());
    assert_eq!(bytes.len(), MeshVert::SIZE_BYTES);
    assert_eq!(MeshVert::from_le(&bytes), Some(vert));
    // Truncated input is a clean None, not a panic.
    assert_eq!(MeshVert::from_le(&bytes[..3]), None);
}

#[test]
fn mesh_face_is_exactly_eight_bytes_wedge_then_material() {
    let face = MeshFace {
        i_wedge: [7, 8, 9],
        material_index: 0x0102,
    };
    let bytes = face.to_le_bytes();
    assert_eq!(bytes.len(), MeshFace::SIZE_BYTES);
    assert_eq!(
        bytes,
        [7, 0, 8, 0, 9, 0, 0x02, 0x01],
        "wedge[0..3] then material, all LE u16"
    );
    assert_eq!(MeshFace::from_le(&bytes), Some(face));
    assert_eq!(MeshFace::from_le(&bytes[..7]), None);
}

#[test]
fn mesh_vert_connect_two_i32_fields() {
    let bytes = [
        0x05, 0x00, 0x00, 0x00, // NumVertTriangles = 5
        0x2C, 0x01, 0x00, 0xF0, // TriangleListOffset = 0xF000012C
    ];
    let connect = MeshVertConnect::from_le(&bytes).expect("8 bytes");
    assert_eq!(connect.num_vert_triangles, 5);
    assert_eq!(connect.triangle_list_offset, 0xF000_012Cu32 as i32);
    assert_eq!(MeshVertConnect::SIZE_BYTES, 8);
}
