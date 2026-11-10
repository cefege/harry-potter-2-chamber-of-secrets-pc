#!/usr/bin/env python3
"""Data-none contracts for modern resolution fixture generation; no app launch."""
import unittest
import InputScriptSmoke as replay
import RetailModernResolutionFixtures as fixtures


class ModernResolutionFixtureTests(unittest.TestCase):
    def setUp(self):
        # Synthetic points test serialization only, not real UI hit targets.
        self.args = ((680, 455), (320, 178), (250, 233), (520, 420), (760, 420), (1088, 546))

    def test_matrix_fixtures_compile_and_are_launch_safe(self):
        scripts = fixtures.build_fixtures(*self.args, scroll=[(410, 280), (410, 280)])
        self.assertEqual(set(scripts), {'accept.txt', 'reject.txt', 'timeout.txt', 'restart-accepted.txt', 'restart-restored.txt'})
        for script in scripts.values():
            tick, frames = replay.parse_fixture(script)
            replay.validate_launch_safe_frames(frames)
            self.assertTrue(replay.compile_replay('PrivetDr.unr', tick, frames))
        self.assertEqual(scripts['accept.txt'].count('mouse 410 280'), 2)

    def test_timeout_has_no_input_after_selection_and_exceeds_20_seconds(self):
        script = fixtures.build_fixtures(*self.args)['timeout.txt']
        self.assertTrue(script.endswith('mouse 250 233\npress LeftMouse\nidle 40\nidle 760\n'))
        tick, frames = replay.parse_fixture(script)
        self.assertGreater(760 * tick, 20)
        self.assertTrue(all(not frame for frame in frames[-760:]))

    def test_choice_and_post_resize_points_must_fit_their_viewports(self):
        with self.assertRaises(ValueError):
            fixtures.build_fixtures(*self.args[:2], (800, 2), *self.args[3:])
        with self.assertRaises(ValueError):
            fixtures.build_fixtures(*self.args[:3], (2, 720), *self.args[4:])
        with self.assertRaises(ValueError):
            fixtures.build_fixtures(*self.args, scroll=[(0, 600)])

    def test_separate_decisions_and_restart_targets(self):
        scripts = fixtures.build_fixtures(*self.args)
        self.assertIn('mouse 520 420\npress LeftMouse', scripts['accept.txt'])
        self.assertIn('mouse 760 420\npress LeftMouse', scripts['reject.txt'])
        self.assertNotIn('mouse 520 420', scripts['timeout.txt'])
        self.assertIn('mouse 1088 546', scripts['restart-accepted.txt'])
        self.assertIn('mouse 680 455', scripts['restart-restored.txt'])


if __name__ == '__main__':
    unittest.main()
