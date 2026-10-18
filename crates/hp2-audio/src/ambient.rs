//! Port of the decision logic in `ALAudioSubsystem.cpp`'s ambient handling:
//! `HandleAmbience` (the candidate list, its sort) and `UpdateAmbience` (stop
//! rule). The C++ shell keeps everything that touches engine objects: the actor
//! walk, the `Sources` scan, `IsOwnedBy`, `IsAudibleAt`, region lookups,
//! `SoundPriority`, `PlaySound`, and the source stop. This module only holds the
//! arithmetic and ordering that decides them.

use crate::priority::SF_NO3D;
use crate::sort::ue_sort;

/// `ESoundSlot` `SLOT_Ambient` (`EngineClasses.h:488-493`).
pub const SLOT_AMBIENT: i32 = 4;

/// `SF_Looping` (`UnAudio.h`).
pub const SF_LOOPING: i32 = 2;
/// `SF_UpdatePitch` (`UnAudio.h`).
pub const SF_UPDATE_PITCH: i32 = 32;

/// `Actor->GetIndex()*16 + SLOT_Ambient*2` (`ALAudioSubsystem.cpp:1448`).
pub fn ambient_source_id(actor_index: i32) -> i32 {
    actor_index.wrapping_mul(16).wrapping_add(SLOT_AMBIENT * 2)
}

/// Flags an ambient sound starts with (`ALAudioSubsystem.cpp:1460` owned,
/// `:1480` occluded/distance-gated).
pub fn ambient_flags(owned: bool) -> i32 {
    if owned {
        SF_LOOPING | SF_NO3D | SF_UPDATE_PITCH
    } else {
        SF_LOOPING | SF_UPDATE_PITCH
    }
}

/// `HandleAmbience` admission (`ALAudioSubsystem.cpp:1455-1482`). Owned actors
/// are always admitted; the others need
/// `dist_squared <= Square(radius_multiplier*radius*max_radius_multiplier)`.
/// `radius_multiplier` is the occlusion RM; it is 1.0 when the sound is audible
/// and for owned actors.
pub fn ambient_admitted(
    owned: bool,
    dist_squared: f32,
    radius: f32,
    radius_multiplier: f32,
    max_radius_multiplier: f32,
) -> bool {
    if owned {
        return true;
    }
    let reach = radius_multiplier * radius * max_radius_multiplier;
    dist_squared <= reach * reach
}

/// `UpdateAmbience` stop rule (`ALAudioSubsystem.cpp:1514-1524`): a playing
/// ambient source is stopped once `dist_squared > Square(zone_radius*max)`.
pub fn ambient_out_of_range(
    dist_squared: f32,
    zone_radius: f32,
    max_radius_multiplier: f32,
) -> bool {
    let reach = zone_radius * max_radius_multiplier;
    dist_squared > reach * reach
}

/// Sorts ambient candidates highest priority first, exactly as the original
/// `Sort` over `ALAmbient` does (`ALAudioSubsystem.cpp:1492`, `Compare` at
/// `:1270`). Ties are not stable; the order is the original algorithm's.
pub fn sort_by_priority<T>(items: &mut [T], priority: impl Fn(&T) -> f32) {
    ue_sort(items, |a, b| ambient_compare(priority(a), priority(b)));
}

/// `static INT Compare(ALAmbient& A, ALAmbient& B)`: `1` when
/// `B.Priority - A.Priority >= 0`, otherwise `-1`.
fn ambient_compare(a: f32, b: f32) -> i32 {
    if b - a >= 0.0 { 1 } else { -1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct Entry {
        priority: f32,
        id: i32,
    }

    /// Deterministic LCG; same constants as the C++ oracle in
    /// `/tmp/ambient_ref/ref.cpp` (`case D`).
    fn lcg_entries(n: usize) -> Vec<Entry> {
        let mut x: u32 = 12345;
        (0..n)
            .map(|i| {
                x = x.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fff_ffff;
                Entry {
                    priority: (x % 7) as f32,
                    id: i as i32,
                }
            })
            .collect()
    }

    fn entries(priorities: &[f32]) -> Vec<Entry> {
        priorities
            .iter()
            .enumerate()
            .map(|(i, &p)| Entry {
                priority: p,
                id: i as i32,
            })
            .collect()
    }

    fn order(list: &[Entry]) -> Vec<i32> {
        list.iter().map(|e| e.id).collect()
    }

    fn sorted(mut list: Vec<Entry>) -> Vec<i32> {
        sort_by_priority(&mut list, |e| e.priority);
        order(&list)
    }

    /// Permutations captured from the original C++ `Sort` + `Compare`
    /// (`/tmp/ambient_ref/ref.cpp`, compiled with clang++ -ffp-contract=off).
    /// Equal priorities are deliberately present: the expected order is the
    /// original algorithm's, not a stable sort.
    #[test]
    fn sort_order_with_ties_matches_cpp_quicksort_path() {
        let a = entries(&[
            5., 1., 5., 3., 5., 2., 5., 4., 5., 0., 5., 5., 1., 9., 9., 0.,
        ]);
        assert_eq!(
            sorted(a),
            [13, 14, 8, 0, 10, 2, 6, 4, 11, 7, 3, 5, 1, 12, 9, 15]
        );
    }

    #[test]
    fn all_equal_priorities_keep_cpp_permutation() {
        let b = entries(&[7.; 9]);
        assert_eq!(sorted(b), [4, 1, 2, 3, 0, 5, 6, 7, 8]);
    }

    #[test]
    fn bubble_sort_path_at_eight_elements_matches_cpp() {
        let c = entries(&[3., 1., 2., 5., 4., 0., 7., 6.]);
        assert_eq!(sorted(c), [6, 7, 3, 4, 0, 2, 1, 5]);
    }

    #[test]
    fn lcg_heavy_ties_50_elements_match_cpp() {
        assert_eq!(
            sorted(lcg_entries(50)),
            [
                44, 1, 27, 48, 43, 14, 19, 18, 36, 32, 46, 11, 37, 23, 29, 15, 22, 21, 49, 25, 7,
                2, 47, 45, 0, 30, 26, 38, 41, 31, 35, 40, 24, 33, 3, 16, 8, 28, 13, 5, 10, 9, 42,
                6, 39, 12, 4, 20, 34, 17
            ]
        );
    }

    #[test]
    fn single_element_and_empty_are_untouched() {
        assert_eq!(sorted(entries(&[1.0])), [0]);
        assert_eq!(sorted(Vec::new()), Vec::<i32>::new());
    }

    #[test]
    fn sorted_output_is_non_increasing_in_priority() {
        let mut list = lcg_entries(200);
        sort_by_priority(&mut list, |e| e.priority);
        assert!(list.windows(2).all(|w| w[0].priority >= w[1].priority));
    }

    #[test]
    fn source_id_is_index_times_16_plus_slot_ambient_times_2() {
        assert_eq!(ambient_source_id(0), 8);
        assert_eq!(ambient_source_id(3), 56);
        assert_eq!(ambient_source_id(1000), 16008);
    }

    #[test]
    fn flags_match_owned_and_unowned_starts() {
        assert_eq!(ambient_flags(true), 2 | 16 | 32);
        assert_eq!(ambient_flags(false), 2 | 32);
    }

    /// Cases from `ref.cpp` (`ADMIT`): (owned, dist, radius, rm, mult).
    #[test]
    fn admission_matches_cpp_including_boundaries() {
        let cases: [(bool, f32, f32, f32, f32); 8] = [
            (true, 1e30, 10., 1., 1.),
            (false, 100., 10., 1., 1.),
            (false, 100.00002, 10., 1., 1.),
            (false, 0., 0., 1., 1.),
            (false, 0.5, 0., 1., 1.),
            (false, 1., 10., 0., 1.),
            (false, 1., 10., 1., 0.),
            (false, 4., 2., 0.5, 2.),
        ];
        let expected = [true, true, false, true, false, false, false, true];
        for (k, (case, want)) in cases.iter().zip(expected).enumerate() {
            let &(owned, dist, radius, rm, mult) = case;
            assert_eq!(
                ambient_admitted(owned, dist, radius, rm, mult),
                want,
                "case {k}"
            );
        }
    }

    /// An owned actor is admitted however far away it is; its RM is irrelevant.
    #[test]
    fn owned_actor_bypasses_distance_and_occlusion() {
        assert!(ambient_admitted(true, f32::MAX, 0.0, 0.0, 0.0));
    }

    /// Cases from `ref.cpp` (`OUT`): (dist, zone, mult). `dist > Square(zone*mult)`.
    #[test]
    fn out_of_range_matches_cpp_including_boundaries() {
        assert!(!ambient_out_of_range(100., 10., 1.));
        assert!(ambient_out_of_range(100.00002, 10., 1.));
        assert!(!ambient_out_of_range(0., 0., 1.));
        assert!(ambient_out_of_range(1., 0., 1.));
    }
}
