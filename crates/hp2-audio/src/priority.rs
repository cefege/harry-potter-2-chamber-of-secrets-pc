//! Replaces `UALAudioSubsystem::FindLeastPriorityNonTalkSource`
//! (`ALAudioSubsystem.cpp:2085-2101`).

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

    // Verified against the real C++ `UALAudioSubsystem::
    // FindLeastPriorityNonTalkSource` via a throwaway probe calling
    // the function directly (externally linked -- a `static` member,
    // no anonymous-namespace indirection needed) on the exact three
    // scenarios `Tests/AudioTests.cpp:625-635` already exercises
    // against the real C++: eviction picks the lowest-priority
    // non-talk source, never evicts an existing talk source, and
    // returns INDEX_NONE when every occupied source is a talk source.

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
}
