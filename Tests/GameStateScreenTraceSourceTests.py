#!/usr/bin/env python3
"""Source-order checks for the opt-in screening diagnostic; not runtime proof."""
from pathlib import Path
import re
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
LEVEL = (ROOT / "crates/hp2-engine/src/level.rs").read_text()
LOAD = (ROOT / "crates/hp2-engine/src/game_load_map_owner.rs").read_text()
PERSIST = (ROOT / "crates/hp2-engine/src/level_persist_owner.rs").read_text()


def validate_trace(path, expected_session):
    """Reject partial, stale, corrupt and unbalanced trace transport.

    A valid result covers screening transport only. Capture selection is blocked.
    """
    lines = Path(path).read_bytes().splitlines(keepends=True)
    if not re.fullmatch(r"[0-9a-f]{32}", expected_session):
        raise ValueError("expected session must be 32 lowercase hex digits")
    header = f"event=trace_start version=1 session={expected_session} capture_selection=blocked\n".encode()
    if not lines or lines[0] != header:
        raise ValueError("missing or invalid exclusive-session header")
    footer = re.fullmatch(rb"event=trace_end records=(\d+) hash=([0-9a-f]{16}) capture_selection=blocked\n", lines[-1])
    if len(lines) < 2 or not footer or int(footer[1]) != len(lines) - 2:
        raise ValueError("missing or invalid completion record")
    digest = 0xcbf29ce484222325
    for byte in b"".join(lines[:-1]):
        digest = ((digest ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    if digest != int(footer[2], 16):
        raise ValueError("trace checksum mismatch")
    begins = {b"travel_assign_begin": b"travel_assign_end", b"persistent_begin": b"persistent_end",
              b"screen_begin": b"screen_end", b"resolve_begin": b"resolve_end",
              b"restore_actor_begin": b"restore_actor_serialize_end"}
    ends = set(begins.values())
    observations = {b"restore_actor_end", b"player", b"none_state", b"invalid_state", b"valid_state",
                    b"actor_group", b"actor_exclude", b"decision", b"screen_complete"}
    stack = []
    for index, line in enumerate(lines[1:-1]):
        match = re.match(rb"record=(\d+) event=([a-z_]+) ", line)
        if not match or int(match[1]) != index or not line.endswith(b"\n"):
            raise ValueError("invalid record sequence")
        event = match[2]
        if event not in begins and event not in ends and event not in observations:
            raise ValueError("unknown trace event")
        if event in begins:
            stack.append(begins[event])
        elif event in ends:
            if not stack or stack.pop() != event:
                raise ValueError("unbalanced event boundaries")
    if stack:
        raise ValueError("unfinished event boundaries")
    return "screen_transport_complete; capture_selection=blocked"


SESSION = "0123456789abcdef0123456789abcdef"


def synthetic_trace(records):
    header = f"event=trace_start version=1 session={SESSION} capture_selection=blocked\n".encode()
    content = header + b"".join(records)
    digest = 0xcbf29ce484222325
    for byte in content:
        digest = ((digest ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return content + f"event=trace_end records={len(records)} hash={digest:016x} capture_selection=blocked\n".encode()


class GameStateScreenTraceSourceTests(unittest.TestCase):
    def test_consumer_rejects_stale_partial_corrupt_and_unbalanced(self):
        session = SESSION
        header = f"event=trace_start version=1 session={session} capture_selection=blocked\n".encode()
        trace = synthetic_trace
        good = trace([b"record=0 event=screen_begin level=0x1\n", b"record=1 event=screen_end level=0x1\n"])
        with patch.object(Path, "read_bytes", return_value=good):
            self.assertEqual(validate_trace("unused", session), "screen_transport_complete; capture_selection=blocked")
            with self.assertRaises(ValueError): validate_trace("unused", "f" * 32)
        for broken in (good[:-1], good.replace(b"level=0x1", b"level=0x2", 1),
                       trace([b"record=0 event=screen_begin level=0x1\n"]), header):
            with patch.object(Path, "read_bytes", return_value=broken):
                with self.assertRaises(ValueError): validate_trace("unused", session)

    def test_consumer_accepts_observed_errors_without_claiming_restore_success(self):
        cases = [
            [b"travel_assign_begin returned=pending", b"travel_assign_end returned=false"],
            [b"screen_begin level=0x1", b"resolve_begin actor=0x2", b"resolve_end value=0", b"screen_end returned=false"],
            [b"persistent_begin level=0x1", b"restore_actor_begin actor=0x2",
             b"restore_actor_serialize_end returned=false", b"persistent_end returned=error"],
            # Cursor failure after successful serialization also omits the reset marker.
            [b"persistent_begin level=0x1", b"restore_actor_begin actor=0x2",
             b"restore_actor_serialize_end returned=true", b"persistent_end returned=ok"],
        ]
        for events in cases:
            records = [f"record={i} event=".encode() + event + b"\n" for i, event in enumerate(events)]
            self.assertFalse(any(b"event=restore_actor_end " in record for record in records))
            with self.subTest(events=events), patch.object(Path, "read_bytes", return_value=synthetic_trace(records)):
                self.assertEqual(validate_trace("unused", SESSION), "screen_transport_complete; capture_selection=blocked")
        success = [b"record=0 event=restore_actor_begin actor=0x2\n",
                   b"record=1 event=restore_actor_serialize_end returned=true\n",
                   b"record=2 event=restore_actor_end actor=0x2\n"]
        with patch.object(Path, "read_bytes", return_value=synthetic_trace(success)):
            self.assertEqual(validate_trace("unused", SESSION), "screen_transport_complete; capture_selection=blocked")

    def test_consumer_rejects_unknown_and_mismatched_boundaries(self):
        for events in ([b"unknown_begin x=0", b"unknown_end x=0"],
                       [b"screen_begin x=0", b"persistent_end x=0"],
                       [b"restore_actor_begin x=0", b"restore_actor_end x=0"]):
            records = [f"record={i} event=".encode() + event + b"\n" for i, event in enumerate(events)]
            with self.subTest(events=events), patch.object(Path, "read_bytes", return_value=synthetic_trace(records)):
                with self.assertRaises(ValueError): validate_trace("unused", SESSION)

    def test_travel_failure_observation_precedes_error_propagation(self):
        body = LOAD.split("AssignTravel => {", 1)[1].split("\n                Flush =>", 1)[0]
        self.assertLess(body.index('game_state_trace("travel_assign_begin"'), body.index("let result = if"))
        self.assertLess(body.index("assign_travel_map("), body.index('game_state_trace("travel_assign_end"'))
        self.assertLess(body.index('game_state_trace("travel_assign_end"'), body.index("result?;"))
        self.assertIn("result.is_ok()", body)

    def test_sink_is_opt_in_and_tracks_write_failure(self):
        start = LOAD.split("pub(crate) fn game_state_trace_start", 1)[1].split("\n}", 1)[0]
        body = LOAD.split("pub(crate) fn game_state_trace(event:", 1)[1].split("\n}", 1)[0]
        finish = LOAD.split("pub(crate) fn game_state_trace_finish", 1)[1].split("\n}", 1)[0]
        self.assertIn('var_os("HP2_GAME_STATE_TRACE")', start)
        self.assertIn("create_new(true)", start)
        self.assertNotIn("var_os", body)
        self.assertLess(body.index("return;"), body.index("details()"))
        self.assertIn("session.failed = true", body)
        self.assertIn("!complete || session.failed", finish)
        self.assertIn("event=trace_end", finish)

    def test_screen_return_and_callback_observations_surround_real_calls(self):
        body = LEVEL.split("pub unsafe fn level_screen_actors_body_with", 1)[1].split('/// `ULevel::ShrinkLevel`', 1)[0]
        for event in ("player", "none_state", "invalid_state", "valid_state", "actor_group", "actor_exclude", "decision", "screen_complete"):
            self.assertIn(f'screen_trace("{event}"', body)
        start = body.index('screen_trace("resolve_begin"')
        call = body.index("let result = s.actor_event_on_resolve_game_state(act)", start)
        end = body.index('screen_trace("resolve_end"', call)
        propagation = body.index("result?", end)
        self.assertLess(start, call)
        self.assertLess(call, end)
        self.assertLess(end, propagation)

    def test_restore_observations_surround_actual_serialize(self):
        body = PERSIST.split("pub unsafe fn load_persistent_actors", 1)[1]
        start = body.index('game_state_trace("restore_actor_begin"')
        call = body.index("serialize_persistence(access, actor, &borrowed)", start)
        end = body.index('game_state_trace("restore_actor_serialize_end"', call)
        propagate = body.index("restored?", end)
        reset = body.index("set_b_script_initialized(false)", propagate)
        complete = body.index('game_state_trace("restore_actor_end"', reset)
        self.assertLess(start, call)
        self.assertLess(call, end)
        self.assertLess(end, propagate)
        self.assertLess(propagate, reset)
        self.assertLess(reset, complete)

    def test_no_player_or_invalid_state_does_not_report_a_valid_screen(self):
        body = LEVEL.split("pub unsafe fn level_screen_actors_body_with", 1)[1].split("// *** Screen Actors:", 1)[0]
        none_branch = body.split('screen_trace("none_state"', 1)[1].split("}", 1)[0]
        invalid_branch = body.split('screen_trace("invalid_state"', 1)[1].split("}", 1)[0]
        self.assertIn("return Ok(())", none_branch)
        self.assertIn("screen_invalid_state(pawn)?", invalid_branch)
        self.assertIn("return Ok(())", invalid_branch)
        self.assertNotIn('screen_trace("valid_state"', none_branch + invalid_branch)

    def test_restore_failure_cannot_reach_restore_end(self):
        body = PERSIST.split("pub unsafe fn load_persistent_actors", 1)[1]
        serialize = body.split('game_state_trace("restore_actor_serialize_end"', 1)[1]
        propagation, complete = serialize.split('game_state_trace("restore_actor_end"', 1)
        self.assertIn("restored?;", propagation)
        self.assertIn("if cursor.is_error()?", propagation)
        self.assertIn("break;", propagation)
        self.assertIn("set_b_script_initialized(false)", propagation)
        self.assertIn("name_utf32={name:?}", complete)

    def test_failed_resolve_is_propagated_before_next_actor(self):
        body = LEVEL.split('screen_trace("resolve_begin"', 1)[1].split('screen_trace("screen_complete"', 1)[0]
        self.assertIn("i32::from(result.is_ok())", body)
        self.assertIn("result?;", body)
        self.assertNotIn("unwrap_or", body)

    def test_trace_boundaries_cover_travel_persistence_and_screen(self):
        for event in ("travel_assign_begin", "travel_assign_end", "persistent_begin", "persistent_end", "screen_begin", "screen_end"):
            self.assertIn(f'game_state_trace("{event}"', LOAD)


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--validate":
        try:
            print(validate_trace(sys.argv[2], sys.argv[3]))
        except (OSError, ValueError) as error:
            print(f"incomplete: {error}", file=sys.stderr)
            raise SystemExit(1)
    else:
        unittest.main()
