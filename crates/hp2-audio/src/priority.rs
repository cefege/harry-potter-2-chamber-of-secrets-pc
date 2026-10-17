//! Replaces `UALAudioSubsystem::FindLeastPriorityNonTalkSource`
//! (`ALAudioSubsystem.cpp:2085-2101`) and `UALAudioSubsystem::
//! SoundPriority` (`:1946-1961`).

/// Matches `UnAudio.h:26-39`'s `ESoundFlags` bits this function
/// reads.
const SF_MUSIC: i32 = 8;
const SF_NO3D: i32 = 16;
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
/// ambient sound (`:1404`, `:1424`), from `PlaySound` (`:808`), and
/// from `FindLeastImportantSound` (`:2144`) -- five call sites, not
/// one. `FindLeastImportantSound` itself stays unported (reads
/// `this->Sources` directly, deeply object-graph-coupled); the other
/// four callers already call through `SoundPriority`'s C++ body,
/// which now forwards here unconditionally. No ctest exercises
/// `Update`/`PlaySound`'s runtime call paths -- `audio_lifecycle`
/// only calls this function directly via
/// `TestSoundPriorityFfiBoundary`, so the on-tick behavior is
/// unexercised by any automated check.
pub fn sound_priority(
    dist_squared: f32,
    radius: f32,
    audio_max_radius_multiplier: f32,
    volume: f32,
    flags: i32,
) -> f32 {
    let radius_factor = if radius != 0.0 {
        let denom = (audio_max_radius_multiplier * radius).powi(2);
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

    // Only the literal-mirror test below
    // (`mirrors_the_real_cpp_talk_registration_eviction_policy_test`)
    // is actually checked against `Tests/AudioTests.cpp:612-636`'s
    // `TestTalkRegistrationEvictionPolicy` -- the real, unmodified,
    // already-passing C++ oracle test, already part of the
    // `audio_lifecycle` ctest baseline. The five cases below are
    // NOT drawn from that C++ test (its own three-step sequence
    // doesn't exercise an empty list, a tie, or an unoccupied-source
    // skip); they're derived directly from reading
    // `ALAudioSubsystem.cpp:2085-2101`'s source -- the `Sound`
    // null check, the strict `<` tie-break, and the `INDEX_NONE`
    // starting value. No separate throwaway probe was built for any
    // of this: `FindLeastPriorityNonTalkSource` has external linkage
    // (confirmed via `nm`, a plain `static` method, no anonymous-
    // namespace indirection), but the independent confirmation here
    // is the existing C++ test file for the mirror case, and direct
    // source reading for the other five.

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

    /// Literal mirror of the real, already-passing C++ oracle test
    /// `TestTalkRegistrationEvictionPolicy` (`Tests/AudioTests.cpp:
    /// 612-636`, part of the `audio_lifecycle` ctest), same three
    /// steps, same slot/priority values (`SLOT_Talk=5`,
    /// `SLOT_Ambient=4`, `SLOT_Misc=1` from `EngineClasses.h:488-493`)
    /// and same expected results at each step -- not a new scenario
    /// invented for this port, the exact one the real C++ test suite
    /// already independently verifies.
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

    // sound_priority: derived directly from reading
    // ALAudioSubsystem.cpp:1946-1961's source -- no existing C++
    // oracle test exercises this function (grepped
    // Tests/AudioTests.cpp, no match).

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
}
