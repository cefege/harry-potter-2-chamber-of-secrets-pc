//! Ordered `URender` initialization and destruction policy.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderInitSettings {
    pub num_dyn_light_surfs: i32,
    pub num_dyn_light_leaves: i32,
    pub global_mesh_lod: f32,
    pub global_shape_lod: f32,
    pub global_shape_lod_adjust: f32,
    pub shape_lod_mode: i32,
    pub shape_lod_fix: f32,
    pub max_particle_size: f32,
    pub max_particle_render_time: f32,
    pub average_time: f32,
    pub total_time: f32,
    pub peak_frame_time: f32,
    pub lod_actor_lights: i32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitLeaf {
    DynamicMemory = 0,
    SceneMemory = 1,
    Base = 2,
    GlobalRender = 3,
    ApplySettings = 4,
    Cache = 5,
    VectorMemory = 6,
    Statistics = 7,
    LightManager = 8,
    Log = 9,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestroyLeaf {
    DynamicMemory = 0,
    SceneMemory = 1,
    DynamicLightStorage = 2,
    LightManager = 3,
    VectorMemory = 4,
    Log = 5,
    Base = 6,
}

pub const fn defaults() -> RenderInitSettings {
    RenderInitSettings {
        num_dyn_light_surfs: 0,
        num_dyn_light_leaves: 0,
        global_mesh_lod: 1.0,
        global_shape_lod: 1.0,
        global_shape_lod_adjust: 1.0,
        shape_lod_mode: 1,
        shape_lod_fix: 1.0,
        max_particle_size: 1.0,
        max_particle_render_time: 0.05,
        average_time: -1.0,
        total_time: 0.0,
        peak_frame_time: 0.0,
        lod_actor_lights: 0,
    }
}

pub fn initialize<E>(mut invoke: impl FnMut(InitLeaf) -> Result<(), E>) -> Result<(), E> {
    for leaf in [
        InitLeaf::DynamicMemory,
        InitLeaf::SceneMemory,
        InitLeaf::Base,
        InitLeaf::GlobalRender,
        InitLeaf::ApplySettings,
        InitLeaf::Cache,
        InitLeaf::VectorMemory,
        InitLeaf::Statistics,
        InitLeaf::LightManager,
        InitLeaf::Log,
    ] {
        invoke(leaf)?;
    }
    Ok(())
}

pub fn destroy<E>(mut invoke: impl FnMut(DestroyLeaf) -> Result<(), E>) -> Result<(), E> {
    for leaf in [
        DestroyLeaf::DynamicMemory,
        DestroyLeaf::SceneMemory,
        DestroyLeaf::DynamicLightStorage,
        DestroyLeaf::LightManager,
        DestroyLeaf::VectorMemory,
        DestroyLeaf::Log,
        DestroyLeaf::Base,
    ] {
        invoke(leaf)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialization_applies_legacy_defaults_before_dependent_leaves() {
        let settings = defaults();
        let mut calls = Vec::new();
        initialize(|leaf| {
            calls.push(leaf);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(
            calls,
            vec![
                InitLeaf::DynamicMemory,
                InitLeaf::SceneMemory,
                InitLeaf::Base,
                InitLeaf::GlobalRender,
                InitLeaf::ApplySettings,
                InitLeaf::Cache,
                InitLeaf::VectorMemory,
                InitLeaf::Statistics,
                InitLeaf::LightManager,
                InitLeaf::Log
            ]
        );
        assert_eq!(settings.global_mesh_lod, 1.0);
        assert_eq!(settings.max_particle_render_time, 0.05);
        assert_eq!(settings.average_time, -1.0);
    }

    #[test]
    fn lifecycle_stops_at_first_failed_leaf() {
        let mut calls = Vec::new();
        assert!(
            initialize(|leaf| {
                calls.push(leaf);
                if leaf == InitLeaf::Cache {
                    Err(())
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert_eq!(
            calls,
            vec![
                InitLeaf::DynamicMemory,
                InitLeaf::SceneMemory,
                InitLeaf::Base,
                InitLeaf::GlobalRender,
                InitLeaf::ApplySettings,
                InitLeaf::Cache
            ]
        );

        let mut destroyed = Vec::new();
        destroy(|leaf| {
            destroyed.push(leaf);
            if leaf == DestroyLeaf::LightManager {
                Err(())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(
            destroyed,
            vec![
                DestroyLeaf::DynamicMemory,
                DestroyLeaf::SceneMemory,
                DestroyLeaf::DynamicLightStorage,
                DestroyLeaf::LightManager
            ]
        );
    }
}
