#!/usr/bin/env python3
"""Data-none contracts for the isolated retail settings harness."""
import hashlib
from pathlib import Path
import tempfile
import sys
import unittest

import RetailStockSettingsSmoke as smoke


class RetailStockSettingsContracts(unittest.TestCase):
    def test_ini_override_replaces_duplicates_and_preserves_other_sections(self):
        text = '[One]\nKey=old\nKey=older\nOther=x\n[Two]\nKey=keep\n'
        changed = smoke.set_ini(text, 'One', 'Key', 'new')
        self.assertEqual(changed.count('Key=new'), 1)
        self.assertNotIn('Key=old', changed)
        self.assertIn('Other=x', changed)
        self.assertIn('[Two]\nKey=keep', changed)
        self.assertIn('[Three]\nKey=v', smoke.set_ini(changed, 'Three', 'Key', 'v'))

    def test_pruning_keeps_checkpoints_recent_frames_and_other_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index in range(6):
                (root / f'frame_{index:06}.png').write_bytes(b'frame')
            (root / 'frame_meta.json').write_text('{}')
            (root / 'other.png').write_bytes(b'other')
            removed = smoke.prune_new_frames(root, {1, 5}, finished=False)
            self.assertEqual(set(removed), {'frame_000002.png', 'frame_000003.png'})
            self.assertTrue((root / 'frame_000000.png').exists())
            self.assertTrue((root / 'frame_000004.png').exists())
            removed = smoke.prune_new_frames(root, {1, 5}, finished=True)
            self.assertEqual(removed, [])
            self.assertTrue((root / 'frame_000001.png').exists())
            self.assertTrue((root / 'frame_000005.png').exists())
            self.assertTrue((root / 'frame_meta.json').exists())
            self.assertTrue((root / 'other.png').exists())

    def test_supervised_capture_retains_only_checkpoints(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            frames = root / 'frames'
            code = ('from pathlib import Path; import sys; p=Path(sys.argv[1]); p.mkdir(); '
                    '[(p / ("frame_%06d.png" % i)).write_bytes(b"x") for i in range(8)]')
            result, removed, errors = smoke.supervised_checkpoints(
                [sys.executable, '-c', code, str(frames)], home=root / 'home', cwd=root,
                timeout_seconds=10, directory=frames, keep={2, 6})
            self.assertEqual(result[1], 0)
            self.assertFalse(errors)
            self.assertEqual(len(removed), 4)
            self.assertEqual({p.name for p in frames.iterdir()}, {'frame_000000.png', 'frame_000002.png', 'frame_000006.png', 'frame_000007.png'})

    def test_png_dimensions_checks_header(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'frame.png'
            path.write_bytes(b'\x89PNG\r\n\x1a\n' + b'\x00\x00\x00\x0dIHDR' + (640).to_bytes(4, 'big') + (480).to_bytes(4, 'big'))
            self.assertEqual(smoke.png_dimensions(path), (640, 480))
            path.write_bytes(b'not a PNG')
            with self.assertRaises(ValueError):
                smoke.png_dimensions(path)

    def test_identity_detects_changes_and_rejects_escape(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data = root / 'data'
            data.mkdir()
            (data / 'file').write_bytes(b'abc')
            inventory = root / 'inventory'
            digest = hashlib.sha256(b'abc').hexdigest()
            inventory.write_text(f'file\t3\t{digest}\n')
            self.assertTrue(smoke.identity(data, inventory)[0]['matches'])
            (data / 'file').write_bytes(b'abd')
            self.assertFalse(smoke.identity(data, inventory)[0]['matches'])
            (root / 'outside').write_bytes(b'abc')
            inventory.write_text(f'../outside\t3\t{digest}\n')
            with self.assertRaises(ValueError):
                smoke.identity(data, inventory)


if __name__ == '__main__':
    unittest.main()
