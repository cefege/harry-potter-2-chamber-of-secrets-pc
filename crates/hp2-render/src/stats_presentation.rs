//! `URender::DrawStats` panel ordering and flag-selection policy.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderStatsPresentationState {
    pub fps: bool,
    pub global: bool,
    pub net: bool,
    pub hardware: bool,
    pub mesh: bool,
    pub particle: bool,
    pub zone: bool,
    pub occlusion: bool,
    pub game: bool,
    pub cache: bool,
    pub eari: bool,
    pub eari_details: bool,
    pub anim: bool,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderStatsPresentationLeaf {
    SetColor = 0,
    Fps = 1,
    SetStatsCursor = 2,
    Global = 3,
    Net = 4,
    FinishNet = 5,
    Hardware = 6,
    Mesh = 7,
    Particle = 8,
    Zone = 9,
    Occlusion = 10,
    Game = 11,
    Cache = 12,
    Eari = 13,
    EariDetails = 14,
    Anim = 15,
}

pub fn present<E>(
    state: &RenderStatsPresentationState,
    mut invoke: impl FnMut(RenderStatsPresentationLeaf) -> Result<(), E>,
) -> Result<(), E> {
    invoke(RenderStatsPresentationLeaf::SetColor)?;
    if state.fps {
        invoke(RenderStatsPresentationLeaf::Fps)?;
    }
    invoke(RenderStatsPresentationLeaf::SetStatsCursor)?;
    if state.global {
        invoke(RenderStatsPresentationLeaf::Global)?;
    }
    if state.net {
        invoke(RenderStatsPresentationLeaf::Net)?;
    }
    invoke(RenderStatsPresentationLeaf::FinishNet)?;
    if state.hardware {
        invoke(RenderStatsPresentationLeaf::Hardware)?;
    }
    if state.mesh {
        invoke(RenderStatsPresentationLeaf::Mesh)?;
    }
    if state.particle {
        invoke(RenderStatsPresentationLeaf::Particle)?;
    }
    if state.zone {
        invoke(RenderStatsPresentationLeaf::Zone)?;
    }
    if state.occlusion {
        invoke(RenderStatsPresentationLeaf::Occlusion)?;
    }
    if state.game {
        invoke(RenderStatsPresentationLeaf::Game)?;
    }
    if state.cache {
        invoke(RenderStatsPresentationLeaf::Cache)?;
    }
    if state.eari {
        invoke(RenderStatsPresentationLeaf::Eari)?;
    }
    if state.eari_details {
        invoke(RenderStatsPresentationLeaf::EariDetails)?;
    }
    if state.anim {
        invoke(RenderStatsPresentationLeaf::Anim)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> RenderStatsPresentationState {
        RenderStatsPresentationState {
            fps: true,
            global: false,
            net: true,
            hardware: true,
            mesh: true,
            particle: true,
            zone: true,
            occlusion: true,
            game: true,
            cache: true,
            eari: true,
            eari_details: true,
            anim: true,
        }
    }
    #[test]
    fn runs_enabled_panels_in_legacy_order() {
        let mut leaves = Vec::new();
        present(&state(), |leaf| {
            leaves.push(leaf);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(
            leaves,
            [
                RenderStatsPresentationLeaf::SetColor,
                RenderStatsPresentationLeaf::Fps,
                RenderStatsPresentationLeaf::SetStatsCursor,
                RenderStatsPresentationLeaf::Net,
                RenderStatsPresentationLeaf::FinishNet,
                RenderStatsPresentationLeaf::Hardware,
                RenderStatsPresentationLeaf::Mesh,
                RenderStatsPresentationLeaf::Particle,
                RenderStatsPresentationLeaf::Zone,
                RenderStatsPresentationLeaf::Occlusion,
                RenderStatsPresentationLeaf::Game,
                RenderStatsPresentationLeaf::Cache,
                RenderStatsPresentationLeaf::Eari,
                RenderStatsPresentationLeaf::EariDetails,
                RenderStatsPresentationLeaf::Anim
            ]
        );
    }
    #[test]
    fn stops_at_the_first_native_leaf_failure() {
        let mut leaves = Vec::new();
        assert_eq!(
            present(&state(), |leaf| {
                leaves.push(leaf);
                (leaf != RenderStatsPresentationLeaf::Mesh)
                    .then_some(())
                    .ok_or(())
            }),
            Err(())
        );
        assert_eq!(leaves.last(), Some(&RenderStatsPresentationLeaf::Mesh));
    }
}
