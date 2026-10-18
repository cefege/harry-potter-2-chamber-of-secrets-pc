//! Replaces `UALAudioSubsystem::FindLeastPriorityNonTalkSource`
//! (`ALAudioSubsystem.cpp:2085-2101`) and `UALAudioSubsystem::
//! SoundPriority` (`:1946-1961`).

/// Matches `UnAudio.h:26-39`'s `ESoundFlags` bits this function
/// reads.
const SF_MUSIC: i32 = 8;
pub(crate) const SF_NO3D: i32 = 16;
const SF_HAS_LIP_SYNC: i32 = 128;

/// Matches `UnTemplate.h:123-126`'s `Clamp(X,Min,Max) = X<Min ? Min :
/// X<Max ? X : Max` bit-for-bit, including its NaN behavior: both
/// comparisons are false for a NaN `x`, so it falls through to
/// `max`. `f32::clamp` is NOT equivalent here -- it propagates NaN
/// unchanged instead, which would diverge from the real C++ whenever
/// `dist_squared / denom` is NaN (e.g. a 0/0 from a zero
/// `audio_max_radius_multiplier`).
fn ue_clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// Replaces `SoundPriority`. `dist_squared` is the real C++'s own
/// `FDistSquared(Location, ViewTarget->Location)` -- computed in C++
/// since it needs live `AActor`/`FVector` state this crate has no
/// FFI boundary for yet (see this crate's module doc). `radius` of
/// `0.0` matches the real C++'s `if (Radius)` false branch exactly
/// (an exact float comparison against zero, not an epsilon check).
///
/// Called on every tick for every active source
/// (`ALAudioSubsystem.cpp:1703`, inside `Update`), for every gathered
/// ambient sound (`:1404`, `:1424`), and from `PlaySound` (`:808`) --
/// four C++ call sites. `FindLeastImportantSound` (`:2105-2156`) also
/// computes a priority per candidate, but calls this Rust function
/// directly (see `find_least_important_sound` below), not through
/// `SoundPriority`'s C++ body, since its own body was replaced
/// entirely by a Rust forward. The `sound_priority` and
/// `find_least_important_sound` values are pinned against the
/// original C++ (pure tree 23ac187) by
/// `/tmp/hp2-m2/audio/priority/priority_oracle.cpp`, which runs the
/// verbatim original bodies over the same inputs as the tests here.
pub fn sound_priority(
    dist_squared: f32,
    radius: f32,
    audio_max_radius_multiplier: f32,
    volume: f32,
    flags: i32,
) -> f32 {
    let radius_factor = if radius != 0.0 {
        let m = audio_max_radius_multiplier * radius;
        let denom = m * m;
        ue_clamp(1.0 - dist_squared / denom, 0.01, 1.0)
    } else {
        1.0
    };
    volume * radius_factor
        + if flags & SF_HAS_LIP_SYNC != 0 {
            4.0
        } else {
            0.0
        }
        + if flags & SF_MUSIC != 0 { 2.0 } else { 0.0 }
        + if flags & SF_NO3D != 0 { 1.0 } else { 0.0 }
}

/// Matches `EngineClasses.h:493`'s `SLOT_Talk = 5` (the
/// `ESoundSlot` enum `Actor.uc` mirrors as `SLOT_Talk`).
const SLOT_TALK: i32 = 5;

/// Matches `Core.h:104`'s `INDEX_NONE = -1`.
const INDEX_NONE: i32 = -1;

/// Mirrors the `ALSource`/`USound` fields `FindLeastImportantSound`
/// needs, plus the inputs `sound_priority` takes once a source's
/// `DistSquared` is already known. `sound` is the source's `Sound`
/// pointer's raw identity bits (`(uintptr_t)Source.Sound` in C++) --
/// Rust only ever compares it for equality, never dereferences it;
/// `0` represents a null `Sound` pointer, matching the real C++'s own
/// `if (pMaxCountSound)` truthiness check on the same field.
pub struct LeastImportantSlot {
    pub sound: usize,
    pub id: i32,
    pub dist_squared: f32,
    pub volume: f32,
    pub radius: f32,
    pub flags: i32,
}

/// Replaces `FindLeastImportantSound` (`ALAudioSubsystem.cpp:
/// 2105-2156`). `slots` must be in the same order as the real C++'s
/// `Sources` array -- the "most frequent sound" tie-break is "first
/// source whose running instance count strictly exceeds the current
/// max," decided inline during a single forward pass over `Sources`,
/// NOT derived from `TMap` iteration/hash order (the real C++ never
/// observes that order either: the comparison happens in the same
/// loop that updates the running count, before any map iteration
/// could occur).
pub fn find_least_important_sound(
    slots: &[LeastImportantSlot],
    incoming_sound: usize,
    audio_max_radius_multiplier: f32,
) -> i32 {
    const INITIAL_MIN_COUNT: i32 = 3;
    let mut counts: std::collections::HashMap<usize, i32> = std::collections::HashMap::new();
    let mut max_count = INITIAL_MIN_COUNT;
    let mut max_count_sound: usize = 0;
    for slot in slots {
        if slot.id != 0 {
            let count = counts.entry(slot.sound).or_insert(0);
            *count += 1;
            if *count > max_count {
                max_count = *count;
                max_count_sound = slot.sound;
            }
        }
    }

    let mut result = INDEX_NONE;
    if max_count_sound != 0 && max_count_sound != incoming_sound {
        let mut lowest_priority = 1000.0f32;
        for (index, slot) in slots.iter().enumerate() {
            if slot.id != 0 && (slot.id & 14) != (SLOT_TALK * 2) && slot.sound == max_count_sound {
                let priority = sound_priority(
                    slot.dist_squared,
                    slot.radius,
                    audio_max_radius_multiplier,
                    slot.volume,
                    slot.flags,
                );
                if priority < lowest_priority {
                    lowest_priority = priority;
                    result = index as i32;
                }
            }
        }
    }
    result
}

/// Replaces the `ALSource` fields `FindLeastPriorityNonTalkSource`
/// actually reads: `Sound` (used only for a null check), `Id`, and
/// `Priority`. Every other `ALSource` field (`Source`, `Actor`,
/// `Location`, `Radius`, `ZoneRadius`, ...) is untouched by this
/// function and intentionally not part of this type.
pub struct SourceSlot {
    pub has_sound: bool,
    pub id: i32,
    pub priority: f32,
}

/// Replaces `FindLeastPriorityNonTalkSource`: the index of the
/// lowest-priority source that is not currently playing in the
/// `SLOT_Talk` slot (so talk audio is never evicted to make room for
/// a new sound), or `INDEX_NONE` if every occupied source is a talk
/// source (or no source is occupied at all).
pub fn find_least_priority_non_talk_source(sources: &[SourceSlot]) -> i32 {
    let mut result = INDEX_NONE;
    let mut lowest_priority = 0.0f32;
    for (index, source) in sources.iter().enumerate() {
        if source.has_sound
            && (source.id & 14) != (SLOT_TALK * 2)
            && (result == INDEX_NONE || source.priority < lowest_priority)
        {
            result = index as i32;
            lowest_priority = source.priority;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(has_sound: bool, id: i32, priority: f32) -> SourceSlot {
        SourceSlot {
            has_sound,
            id,
            priority,
        }
    }

    // The mirror test below is a literal copy of the deleted C++ test
    // `TestTalkRegistrationEvictionPolicy` (`git show
    // 06bfdd3:Tests/AudioTests.cpp`, the `audio_lifecycle` ctest). The
    // other cases are pinned against the original pure-C++
    // `FindLeastPriorityNonTalkSource` (23ac187
    // `HarryPotter2/Unreal/ALAudio/Src/ALAudioSubsystem.cpp:2085-2101`)
    // by `/tmp/hp2-m2/audio/priority/priority_oracle.out` (produced by
    // `clang++ -std=c++17 -w -O0 -ffp-contract=off priority_oracle.cpp`).

    #[test]
    fn empty_sources_returns_index_none() {
        assert_eq!(find_least_priority_non_talk_source(&[]), -1);
    }

    #[test]
    fn picks_the_lowest_priority_non_talk_source() {
        // Id values use `SLOT_Talk * 2` (10) in bits 1-3 to mark a
        // talk source, matching the real `(Id & 14) == (SLOT_Talk *
        // 2)` test; non-talk ids below just need to not collide with
        // that pattern.
        let sources = [
            slot(true, 0, 5.0),
            slot(true, 10, 100.0), // talk source: never a candidate
            slot(true, 2, 1.0),    // lowest priority non-talk source
        ];
        assert_eq!(find_least_priority_non_talk_source(&sources), 2);
    }

    #[test]
    fn never_evicts_an_existing_talk_source() {
        let sources = [slot(true, 10, 1.0), slot(true, 10, 0.5)];
        assert_eq!(find_least_priority_non_talk_source(&sources), -1);
    }

    #[test]
    fn unoccupied_sources_are_skipped() {
        let sources = [slot(false, 0, 1.0), slot(true, 2, 5.0)];
        assert_eq!(find_least_priority_non_talk_source(&sources), 1);
    }

    #[test]
    fn first_occupied_non_talk_source_wins_a_tie() {
        let sources = [slot(true, 2, 3.0), slot(true, 4, 3.0)];
        // Only `<` (strictly lower), not `<=`, replaces the running
        // minimum -- the real C++ keeps the first index on a tie,
        // matching `Source.Priority < LowestPriority` exactly.
        assert_eq!(find_least_priority_non_talk_source(&sources), 0);
    }

    /// Literal copy of the deleted C++ test
    /// `TestTalkRegistrationEvictionPolicy` (`git show
    /// 06bfdd3:Tests/AudioTests.cpp`, `audio_lifecycle` ctest): same
    /// three steps and same slot/priority values (`SLOT_Talk=5`,
    /// `SLOT_Ambient=4`, `SLOT_Misc=1` from `EngineClasses.h:488-493`).
    #[test]
    fn mirrors_the_real_cpp_talk_registration_eviction_policy_test() {
        const SLOT_MISC: i32 = 1;
        const SLOT_AMBIENT: i32 = 4;
        let mut sources = [
            slot(true, SLOT_TALK * 2, 100.0),
            slot(true, SLOT_AMBIENT * 2, 2.0),
            slot(true, SLOT_MISC * 2, 1.0),
        ];
        assert_eq!(find_least_priority_non_talk_source(&sources), 2);
        sources[2].id = SLOT_TALK * 2;
        assert_eq!(find_least_priority_non_talk_source(&sources), 1);
        sources[1].id = SLOT_TALK * 2;
        assert_eq!(find_least_priority_non_talk_source(&sources), INDEX_NONE);
    }

    // sound_priority: pinned by
    // /tmp/hp2-m2/audio/priority/priority_oracle.out (lines `prio ...`),
    // which runs the original C++ `SoundPriority` (23ac187
    // `ALAudioSubsystem.cpp:1946-1961`) over these same inputs.

    #[test]
    fn zero_radius_skips_distance_entirely() {
        // Real C++'s `if (Radius)` is an exact float comparison, so
        // RadiusFactor is always 1.0 here regardless of dist_squared.
        assert_eq!(sound_priority(999_999.0, 0.0, 100.0, 10.0, 0), 10.0);
    }

    #[test]
    fn radius_factor_is_one_at_zero_distance() {
        assert_eq!(sound_priority(0.0, 10.0, 100.0, 5.0, 0), 5.0);
    }

    #[test]
    fn radius_factor_floors_at_0_01_past_the_radius() {
        // denom = (100*10)^2 = 1_000_000; dist_squared at or beyond
        // that makes 1 - dist/denom <= 0, clamped to the 0.01 floor.
        assert_eq!(sound_priority(1_000_000.0, 10.0, 100.0, 5.0, 0), 5.0 * 0.01);
        assert_eq!(sound_priority(2_000_000.0, 10.0, 100.0, 5.0, 0), 5.0 * 0.01);
    }

    #[test]
    fn radius_factor_is_correct_in_the_unclamped_middle_of_the_formula() {
        // denom = (100*10)^2 = 1_000_000 (exact in f32); dist_squared
        // = 250_000 -> 1 - 0.25 = 0.75 (exact), never touching either
        // clamp bound. Catches a wrong denominator (e.g. forgetting
        // to square it, or squaring only radius) that the
        // zero/zero-distance/past-the-radius cases above cannot.
        assert_eq!(sound_priority(250_000.0, 10.0, 100.0, 4.0, 0), 3.0);
    }

    #[test]
    fn nan_radius_factor_input_clamps_to_one_not_nan() {
        // Real C++'s Clamp(X,Min,Max) = X<Min ? Min : X<Max ? X : Max
        // both evaluates false for a NaN X, falling through to Max
        // (1.0). f32::clamp would propagate NaN instead -- diverges.
        // Forced here via a zero audio_max_radius_multiplier, making
        // denom 0.0 and 0.0/0.0 (dist_squared is also 0.0) NaN.
        assert_eq!(sound_priority(0.0, 10.0, 0.0, 5.0, 0), 5.0);
    }
    #[test]
    fn flag_bonuses_match_the_real_cpp_weights_and_stack() {
        const SF_MUSIC: i32 = 8;
        const SF_NO3D: i32 = 16;
        const SF_HAS_LIP_SYNC: i32 = 128;
        assert_eq!(sound_priority(0.0, 0.0, 100.0, 0.0, SF_HAS_LIP_SYNC), 4.0);
        assert_eq!(sound_priority(0.0, 0.0, 100.0, 0.0, SF_MUSIC), 2.0);
        assert_eq!(sound_priority(0.0, 0.0, 100.0, 0.0, SF_NO3D), 1.0);
        assert_eq!(
            sound_priority(0.0, 0.0, 100.0, 0.0, SF_HAS_LIP_SYNC | SF_MUSIC | SF_NO3D),
            7.0
        );
    }

    // find_least_important_sound: pinned by
    // /tmp/hp2-m2/audio/priority/priority_oracle.out (lines `li ...`),
    // which runs the original C++ `FindLeastImportantSound` (23ac187
    // `ALAudioSubsystem.cpp:2105-2156`) over these same inputs. Every
    // slot below uses radius=0.0, flags=0 so sound_priority(slot) ==
    // slot.volume exactly, letting volume double as a priority proxy.

    fn li_slot(sound: usize, id: i32, volume: f32) -> LeastImportantSlot {
        LeastImportantSlot {
            sound,
            id,
            dist_squared: 0.0,
            volume,
            radius: 0.0,
            flags: 0,
        }
    }

    #[test]
    fn empty_slots_returns_index_none() {
        assert_eq!(find_least_important_sound(&[], 0, 100.0), INDEX_NONE);
    }

    #[test]
    fn a_sound_at_or_below_the_initial_min_count_is_never_evicted() {
        const SLOT_MISC: i32 = 1;
        // Exactly 3 instances (== INITIAL_MIN_COUNT, not > it) must
        // never trigger eviction consideration.
        let slots = [
            li_slot(0xAAAA, SLOT_MISC * 2, 1.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 2.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 3.0),
        ];
        assert_eq!(find_least_important_sound(&slots, 0, 100.0), INDEX_NONE);
    }

    #[test]
    fn picks_the_lowest_priority_instance_of_the_most_frequent_sound() {
        const SLOT_MISC: i32 = 1;
        const SLOT_TALK_ID: i32 = SLOT_TALK * 2;
        let slots = [
            li_slot(0xAAAA, SLOT_MISC * 2, 10.0), // index 0
            li_slot(0xAAAA, SLOT_MISC * 2, 5.0),  // index 1: lowest eligible
            li_slot(0xAAAA, SLOT_TALK_ID, 1.0), // index 2: lower still, but SLOT_Talk is protected
            li_slot(0xAAAA, SLOT_MISC * 2, 5.0), // index 3: ties index 1, must not win
            li_slot(0xBBBB, SLOT_MISC * 2, 0.5), // index 4: a different sound, never exceeds the count
        ];
        assert_eq!(find_least_important_sound(&slots, 0, 100.0), 1);
    }

    #[test]
    fn the_incoming_sound_is_never_evicted_for_itself() {
        const SLOT_MISC: i32 = 1;
        let slots = [
            li_slot(0xAAAA, SLOT_MISC * 2, 10.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 5.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 1.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 2.0),
        ];
        assert_eq!(
            find_least_important_sound(&slots, 0xAAAA, 100.0),
            INDEX_NONE
        );
    }

    #[test]
    fn null_sound_identity_groups_never_trigger_eviction() {
        // Multiple occupied slots with a null Sound pointer collide
        // into the same `0` map key, same as C++'s TMap<USound*,INT>
        // would, but `if (pMaxCountSound)` is a pointer truthiness
        // check in the real C++ -- a `NULL` max-count "sound" must
        // never be treated as evictable.
        const SLOT_MISC: i32 = 1;
        let slots = [
            li_slot(0, SLOT_MISC * 2, 10.0),
            li_slot(0, SLOT_MISC * 2, 5.0),
            li_slot(0, SLOT_MISC * 2, 1.0),
            li_slot(0, SLOT_MISC * 2, 2.0),
        ];
        assert_eq!(
            find_least_important_sound(&slots, 0xFFFF, 100.0),
            INDEX_NONE
        );
    }

    #[test]
    fn the_first_sound_to_strictly_exceed_the_running_max_wins_not_a_later_equal_count() {
        const SLOT_MISC: i32 = 1;
        // 0xAAAA reaches count 4 first (indices 0-3); 0xBBBB also
        // reaches count 4 later (indices 4-7), but 4 is not > the
        // max_count of 4 already set by 0xAAAA, so 0xBBBB must never
        // displace it -- matching the real C++'s strict `>` test
        // against the *running* max, not a final tally comparison.
        let slots = [
            li_slot(0xAAAA, SLOT_MISC * 2, 9.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 9.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 9.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 1.0), // index 3: the pick
            li_slot(0xBBBB, SLOT_MISC * 2, 0.1),
            li_slot(0xBBBB, SLOT_MISC * 2, 0.1),
            li_slot(0xBBBB, SLOT_MISC * 2, 0.1),
            li_slot(0xBBBB, SLOT_MISC * 2, 0.1), // would win on priority alone, must be ignored
        ];
        assert_eq!(find_least_important_sound(&slots, 0, 100.0), 3);
    }

    #[test]
    fn unoccupied_slots_are_skipped_in_both_passes() {
        const SLOT_MISC: i32 = 1;
        let slots = [
            li_slot(0xAAAA, 0, 0.1), // Id == 0: unoccupied, skipped entirely
            li_slot(0xAAAA, SLOT_MISC * 2, 10.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 9.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 8.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 7.0),
        ];
        // Only 4 occupied instances (index 0 excluded), so count
        // reaches 4 at index 4, same winner-selection as before.
        assert_eq!(find_least_important_sound(&slots, 0, 100.0), 4);
    }

    // Oracle: `priority_oracle.cpp` (`li all_at_1000_no_eviction`,
    // `li one_below_1000_wins`), run against `orig_least_important.inc`
    // (23ac187 `ALAudioSubsystem.cpp:2105-2156`). The original starts
    // `fLowestPriority` at 1000.f and only accepts `fPriority < fLowestPriority`.
    #[test]
    fn no_candidate_below_1000_means_no_eviction() {
        const SLOT_MISC: i32 = 1;
        let slots = [
            li_slot(0xAAAA, SLOT_MISC * 2, 1000.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 1500.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 1000.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 2000.0),
        ];
        assert_eq!(find_least_important_sound(&slots, 0, 100.0), INDEX_NONE);
    }

    #[test]
    fn the_first_candidate_strictly_below_1000_wins() {
        const SLOT_MISC: i32 = 1;
        let slots = [
            li_slot(0xAAAA, SLOT_MISC * 2, 1000.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 999.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 1000.0),
            li_slot(0xAAAA, SLOT_MISC * 2, 1000.0),
        ];
        assert_eq!(find_least_important_sound(&slots, 0, 100.0), 1);
    }
}
