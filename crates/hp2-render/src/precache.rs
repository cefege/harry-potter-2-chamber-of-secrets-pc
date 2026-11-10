//! `URender::Precache` traversal policy.

/// Runs the model-surface then global-texture precache passes.
///
/// The Engine render owner supplies object iteration, texture locks and device
/// calls; this policy controls their ordering and material-flag selection.
pub fn precache(mut query: impl FnMut(u32, i32) -> Result<i32, ()>) -> Result<(), ()> {
    // Model iterator: 0 done, 1 surface count, 2 texture present, 3 surface
    // flags, 4 OR flags into texture, 5 parametric, 6 lock/precache/unlock,
    // 7 next model.
    while query(0, 0)? == 0 {
        let surface_count = query(1, 0)?;
        for index in 0..surface_count {
            if query(2, index)? != 0 {
                let flags = query(3, index)?;
                if flags != 0 {
                    query(4, index)?;
                }
                if query(5, index)? == 0 {
                    query(6, index)?;
                }
            }
        }
        query(7, 0)?;
    }

    // Texture iterator: 8 done, 9 parametric, 10 lock/precache/unlock,
    // 11 next texture.
    while query(8, 0)? == 0 {
        if query(9, 0)? == 0 {
            query(10, 0)?;
        }
        query(11, 0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::precache;

    #[test]
    fn precache_preserves_model_surface_then_texture_iterator_order() {
        let mut calls = Vec::new();
        let mut model_done_checks = 0;
        let mut texture_done_checks = 0;
        precache(|op, index| {
            calls.push((op, index));
            let value = match op {
                0 => {
                    let done = model_done_checks != 0;
                    model_done_checks += 1;
                    done as i32
                }
                1 => 2,
                2 => (index == 0) as i32,
                3 => 1,
                5 => 0,
                7 => 0,
                8 => {
                    let done = texture_done_checks != 0;
                    texture_done_checks += 1;
                    done as i32
                }
                9 => 0,
                11 => 0,
                _ => 0,
            };
            Ok(value)
        })
        .unwrap();
        assert_eq!(
            calls,
            [
                (0, 0),
                (1, 0),
                (2, 0),
                (3, 0),
                (4, 0),
                (5, 0),
                (6, 0),
                (2, 1),
                (7, 0),
                (0, 0),
                (8, 0),
                (9, 0),
                (10, 0),
                (11, 0),
                (8, 0)
            ]
        );
    }

    #[test]
    fn precache_stops_at_each_failed_query_without_later_callbacks() {
        // One model, one present non-parametric surface with flags, then one
        // non-parametric global texture. Each item is (operation, index, result).
        let script = [
            (0, 0, 0),
            (1, 0, 1),
            (2, 0, 1),
            (3, 0, 1),
            (4, 0, 0),
            (5, 0, 0),
            (6, 0, 0),
            (7, 0, 0),
            (0, 0, 1),
            (8, 0, 0),
            (9, 0, 0),
            (10, 0, 0),
            (11, 0, 0),
            (8, 0, 1),
        ];
        for fail_at in 0..script.len() {
            let mut calls = Vec::new();
            let result = precache(|op, index| {
                let position = calls.len();
                calls.push((op, index));
                let &(expected_op, expected_index, value) = script
                    .get(position)
                    .expect("precache made an unexpected later callback");
                assert_eq!((op, index), (expected_op, expected_index));
                if position == fail_at {
                    Err(())
                } else {
                    Ok(value)
                }
            });
            assert_eq!(result, Err(()), "query {fail_at} must fail");
            let expected: Vec<_> = script[..=fail_at]
                .iter()
                .map(|&(op, index, _)| (op, index))
                .collect();
            assert_eq!(calls, expected, "callback prefix at query {fail_at}");
        }
    }

    #[test]
    fn precache_skips_missing_and_parametric_textures() {
        let mut calls = Vec::new();
        let mut model_done_checks = 0;
        let mut texture_done_checks = 0;
        precache(|op, index| {
            calls.push((op, index));
            let value = match op {
                0 => {
                    let done = model_done_checks != 0;
                    model_done_checks += 1;
                    done as i32
                }
                1 => 2,
                2 => (index == 1) as i32,
                3 => 0,
                5 => 1,
                7 => 0,
                8 => {
                    let done = texture_done_checks != 0;
                    texture_done_checks += 1;
                    done as i32
                }
                9 => 1,
                11 => 0,
                _ => 0,
            };
            Ok(value)
        })
        .unwrap();
        assert_eq!(
            calls,
            [
                (0, 0),
                (1, 0),
                (2, 0),
                (2, 1),
                (3, 1),
                (5, 1),
                (7, 0),
                (0, 0),
                (8, 0),
                (9, 0),
                (11, 0),
                (8, 0)
            ]
        );
    }
}
