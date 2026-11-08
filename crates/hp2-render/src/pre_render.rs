//! Ordered `URender::PreRender` policy.

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreRenderLeaf {
    CopyEndTime = 0,
    StartTime = 1,
    ClearCounts = 2,
    MarkMemory = 3,
    EnableFastMath = 4,
    TickRandoms = 5,
}

pub fn run<E>(mut invoke: impl FnMut(PreRenderLeaf) -> Result<(), E>) -> Result<(), E> {
    for leaf in [
        PreRenderLeaf::CopyEndTime,
        PreRenderLeaf::StartTime,
        PreRenderLeaf::ClearCounts,
        PreRenderLeaf::MarkMemory,
        PreRenderLeaf::EnableFastMath,
        PreRenderLeaf::TickRandoms,
    ] {
        invoke(leaf)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_pre_render_leaf_order() {
        let mut calls = Vec::new();
        run(|leaf| {
            calls.push(leaf);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(
            calls,
            vec![
                PreRenderLeaf::CopyEndTime,
                PreRenderLeaf::StartTime,
                PreRenderLeaf::ClearCounts,
                PreRenderLeaf::MarkMemory,
                PreRenderLeaf::EnableFastMath,
                PreRenderLeaf::TickRandoms
            ]
        );
    }

    #[test]
    fn stops_before_random_tick_on_prior_failure() {
        let mut calls = Vec::new();
        assert!(
            run(|leaf| {
                calls.push(leaf);
                if leaf == PreRenderLeaf::MarkMemory {
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
                PreRenderLeaf::CopyEndTime,
                PreRenderLeaf::StartTime,
                PreRenderLeaf::ClearCounts,
                PreRenderLeaf::MarkMemory
            ]
        );
    }
}
