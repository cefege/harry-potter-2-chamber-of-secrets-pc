#!/usr/bin/env python3
"""Data-none contracts for the stock FPS retail replay and evidence parser."""
import hashlib
import os
from pathlib import Path
import tempfile
import unittest
import InputScriptSmoke as replay
import RetailStockFpsSmoke as fps


class RetailStockFpsContracts(unittest.TestCase):
    def test_binary_hash_uses_file_bytes(self):
        with tempfile.TemporaryDirectory(dir=os.environ['HP2_ARTIFACT_DIR']) as root:
            binary = Path(root) / 'binary'
            binary.write_bytes(b'fps binary evidence')
            self.assertEqual(fps.sha256(binary), hashlib.sha256(b'fps binary evidence').hexdigest())

    def test_replays_are_balanced_and_launch_safe(self):
        for fixture in (fps.ENABLE_REOPEN, fps.DISABLE, fps.CHECK_OFF):
            tick, frames = replay.parse_fixture(fixture)
            replay.validate_launch_safe_frames(frames)
            self.assertGreater(tick, 0)
            self.assertTrue(replay.compile_replay('PrivetDr.unr', tick, frames))

    def test_probe_requires_ordered_evidence(self):
        log = '\n'.join([
            'StockFPSProbe tick=3 page=12 control=34 visible=true checked=false focus=false fps=false count=1',
            'StockFPSProbe tick=4 page=12 control=34 visible=true checked=true focus=true fps=true count=1',
            'StockFPSProbe tick=5 page=12 control=34 visible=true checked=true focus=false fps=true count=1',
        ])
        rows = fps.observations(log)
        self.assertEqual(len(rows), 3)
        self.assertEqual(rows[1]['count'], 1)
        self.assertTrue(fps.sequence(rows, [{'fps': False}, {'fps': True, 'focus': True}, {'focus': False}]))
        self.assertFalse(fps.sequence(rows, [{'fps': True}, {'fps': False}]))
        self.assertFalse(fps.sequence([], [{'fps': True}]))


if __name__ == '__main__':
    unittest.main()
