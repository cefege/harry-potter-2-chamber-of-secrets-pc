//! Ordered BSP surface, polygon and node archive traversal.
use std::ffi::c_void;

/// Native struct tag used by the C++ archive entry points.
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum Hp2ModelStructKind {
    Surf,
    Poly,
    Node,
}

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

/// Checked raw archive operations and transient-flag policy for native structs.
#[repr(C)]
pub struct Hp2ModelStructArchiveServices {
    pub io: *const crate::model_archive_io::Hp2ModelArchiveIoServices,
    pub transient_poly_flags: u32,
    pub transient_node_flags: u32,
    pub decals_enabled: bool,
}

/// Archives native BSP structures in their original operation order.
///
/// # Safety
/// `object`, archive, service table, and any referenced arrays must be live for
/// the call. All callbacks are synchronous; no native pointer is retained.
pub unsafe fn serialize(
    object: *mut c_void,
    archive: *mut c_void,
    kind: Hp2ModelStructKind,
    s: &Hp2ModelStructArchiveServices,
) -> i32 {
    use hp2_abi::engine::generated::{FDecal, FPoly};
    use hp2_abi::render_scene::{FBspNode, FBspSurf};

    // SAFETY: caller promises live kind-matched native objects and service table.
    unsafe {
        let io = crate::model_archive_io::NativeModelArchive::new(archive, &*s.io);
        let result = (|| -> Result<(), i32> {
            match kind {
                Hp2ModelStructKind::Surf => {
                    let surf = object.cast::<FBspSurf>();
                    checked(io.object(std::ptr::addr_of_mut!((*surf).texture)))?;
                    checked(io.scalar(std::ptr::addr_of_mut!((*surf).poly_flags)))?;
                    for field in [
                        std::ptr::addr_of_mut!((*surf).p_base),
                        std::ptr::addr_of_mut!((*surf).v_normal),
                        std::ptr::addr_of_mut!((*surf).v_texture_u),
                        std::ptr::addr_of_mut!((*surf).v_texture_v),
                        std::ptr::addr_of_mut!((*surf).i_light_map),
                        std::ptr::addr_of_mut!((*surf).i_brush_poly),
                    ] {
                        checked(io.index(field))?;
                    }
                    checked(io.scalar(std::ptr::addr_of_mut!((*surf).pan_u)))?;
                    checked(io.scalar(std::ptr::addr_of_mut!((*surf).pan_v)))?;
                    checked(io.object(std::ptr::addr_of_mut!((*surf).actor)))?;
                    if io.is_loading() {
                        (*surf).poly_flags &= !s.transient_poly_flags;
                    }
                    if s.decals_enabled && !io.is_loading() && !io.is_saving() {
                        let count = (*surf).decals.array_num;
                        checked(io.count_bytes(
                            count as usize * std::mem::size_of::<FDecal>(),
                            (*surf).decals.array_max as usize * std::mem::size_of::<FDecal>(),
                        ))?;
                        let mut serialized_count = count;
                        checked(io.index(&mut serialized_count))?;
                        let decals = (*surf).decals.data.cast::<FDecal>();
                        for index in 0..count {
                            checked(io.object(std::ptr::addr_of_mut!(
                                (*decals.add(index as usize)).actor
                            )))?;
                        }
                    }
                }
                Hp2ModelStructKind::Poly => {
                    let poly = object.cast::<FPoly>();
                    checked(io.index(std::ptr::addr_of_mut!((*poly).num_vertices)))?;
                    // A package count must not escape the fixed native vertex array.
                    let vertices: *const [hp2_abi::FVector] = std::ptr::addr_of!((*poly).vertex);
                    if (*poly).num_vertices > vertices.len() as i32 { return Err(1); }
                    for vector in [
                        std::ptr::addr_of_mut!((*poly).base),
                        std::ptr::addr_of_mut!((*poly).normal),
                        std::ptr::addr_of_mut!((*poly).texture_u),
                        std::ptr::addr_of_mut!((*poly).texture_v),
                    ] {
                        for component in [
                            std::ptr::addr_of_mut!((*vector).x),
                            std::ptr::addr_of_mut!((*vector).y),
                            std::ptr::addr_of_mut!((*vector).z),
                        ] {
                            checked(io.scalar(component))?;
                        }
                    }
                    let mut index = 0;
                    while index < (*poly).num_vertices {
                        let vertex = std::ptr::addr_of_mut!((*poly).vertex).cast::<hp2_abi::FVector>().add(index as usize);
                        for component in [
                            std::ptr::addr_of_mut!((*vertex).x),
                            std::ptr::addr_of_mut!((*vertex).y),
                            std::ptr::addr_of_mut!((*vertex).z),
                        ] {
                            checked(io.scalar(component))?;
                        }
                        index += 1;
                    }
                    checked(io.scalar(std::ptr::addr_of_mut!((*poly).poly_flags)))?;
                    checked(io.object(std::ptr::addr_of_mut!((*poly).actor)))?;
                    checked(io.object(std::ptr::addr_of_mut!((*poly).texture)))?;
                    checked(io.name(std::ptr::addr_of_mut!((*poly).item_name).cast()))?;
                    checked(io.index(std::ptr::addr_of_mut!((*poly).i_link)))?;
                    checked(io.index(std::ptr::addr_of_mut!((*poly).i_brush_poly)))?;
                    checked(io.scalar(std::ptr::addr_of_mut!((*poly).pan_u)))?;
                    checked(io.scalar(std::ptr::addr_of_mut!((*poly).pan_v)))?;
                    if io.is_loading() {
                        (*poly).poly_flags &= !s.transient_poly_flags;
                    }
                }
                Hp2ModelStructKind::Node => {
                    let node = object.cast::<FBspNode>();
                    for component in [
                        std::ptr::addr_of_mut!((*node).plane.x),
                        std::ptr::addr_of_mut!((*node).plane.y),
                        std::ptr::addr_of_mut!((*node).plane.z),
                        std::ptr::addr_of_mut!((*node).plane.w),
                    ] {
                        checked(io.scalar(component))?;
                    }
                    checked(io.scalar(std::ptr::addr_of_mut!((*node).zone_mask)))?;
                    checked(io.scalar(std::ptr::addr_of_mut!((*node).node_flags)))?;
                    checked(io.index(std::ptr::addr_of_mut!((*node).i_vert_pool)))?;
                    checked(io.index(std::ptr::addr_of_mut!((*node).i_surf)))?;
                    for child in [
                        std::ptr::addr_of_mut!((*node).i_back),
                        std::ptr::addr_of_mut!((*node).i_front),
                        std::ptr::addr_of_mut!((*node).i_plane),
                    ] {
                        checked(io.index(child))?;
                    }
                    checked(io.index(std::ptr::addr_of_mut!((*node).i_collision_bound)))?;
                    checked(io.index(std::ptr::addr_of_mut!((*node).i_render_bound)))?;
                    for zone in 0..2 {
                        checked(io.scalar(std::ptr::addr_of_mut!((*node).i_zone[zone])))?;
                    }
                    checked(io.scalar(std::ptr::addr_of_mut!((*node).num_vertices)))?;
                    for leaf in 0..2 {
                        checked(io.scalar(std::ptr::addr_of_mut!((*node).i_leaf[leaf])))?;
                    }
                    if io.is_loading() {
                        (*node).node_flags &= !(s.transient_node_flags as u8);
                    }
                }
            }
            Ok(())
        })();
        result.err().unwrap_or(0)
    }
}

