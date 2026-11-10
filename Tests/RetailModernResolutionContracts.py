#!/usr/bin/env python3
"""Data-none contract checks for the opt-in retail resolution replay."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

import RetailModernResolutionSmoke as smoke


class ModernResolutionReplayContracts(unittest.TestCase):
    def test_fixture_has_distinct_ordered_observation_points(self):
        fixture, checkpoints = smoke.fixture_and_checkpoints()
        _, frames = smoke.replay.parse_fixture(fixture)
        smoke.replay.validate_launch_safe_frames(frames)
        values = list(checkpoints.values())
        self.assertEqual(values, sorted(values))
        self.assertEqual(len(values), len(set(values)))
        self.assertLess(values[-1], len(frames))

    def test_manifest_checks_content_and_rejects_escape(self):
        root = Path(os.environ['HP2_ARTIFACT_DIR'])
        with tempfile.TemporaryDirectory(prefix='modern-resolution-contract-', dir=root) as temp:
            data = Path(temp) / 'data'
            data.mkdir()
            (data / 'x').write_bytes(b'abc')
            entry = {'path': 'x', 'size': 3, 'sha256': hashlib.sha256(b'abc').hexdigest()}
            manifest = data / 'overlay-manifest.json'
            manifest.write_text(json.dumps({'entries': [entry]}))
            self.assertTrue(smoke.verify_manifest(data)[0]['matches'])
            (data / 'x').write_bytes(b'bad')
            self.assertFalse(smoke.verify_manifest(data)[0]['matches'])
            entry['path'] = '../outside'
            (data.parent / 'outside').write_bytes(b'abc')
            try:
                manifest.write_text(json.dumps({'entries': [entry]}))
                with self.assertRaises(ValueError):
                    smoke.verify_manifest(data)
            finally:
                (data.parent / 'outside').unlink()


if __name__ == '__main__':
    unittest.main()
