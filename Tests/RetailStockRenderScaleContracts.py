#!/usr/bin/env python3
"""Data-free contracts for the opt-in retail scale replay."""
import unittest
import InputScriptSmoke as replay
import RetailStockRenderScaleSmoke as scale


class RetailScaleContracts(unittest.TestCase):
    def test_fixtures_are_safe_and_cover_checkpoints(self):
        for name, path in scale.FIXTURES.items():
            tick, frames = replay.parse_fixture(path.read_text(encoding='utf-8'))
            replay.validate_launch_safe_frames(frames)
            self.assertGreater(tick, 0)
            self.assertGreater(len(frames), max(scale.CHECKPOINTS[name]))

    def test_probe_distinguishes_configured_and_applied_scale(self):
        log = ('StockScaleProbe event=changed percent=50 unavailable=false\n'
               'WgpuDrv: world-scale trace scopes=1 prior_color=0 scaled=0 unavailable=0 requested=50%\n'
               'WgpuDrv: world-scale trace scopes=120 prior_color=0 scaled=119 unavailable=0 requested=50%\n')
        self.assertEqual(scale.probe(log)[0], {'event': 'changed', 'percent': 50, 'unavailable': False})
        rows = scale.gpu_trace(log)
        self.assertEqual(rows[0]['scaled'], 0)
        self.assertEqual(rows[0]['requested'], 50)
        self.assertEqual(rows[1]['scaled'], 119)


if __name__ == '__main__':
    unittest.main()
