#!/usr/bin/env python3
"""Data-free contracts for retail advanced graphics verification."""
import os
from pathlib import Path
import tempfile
import unittest

import RetailStockAdvancedGraphicsSmoke as smoke
import RetailStockGraphicsGpuSmoke as gpu
import InputScriptSmoke as replay

class Contracts(unittest.TestCase):
    def test_page_open_calibration_fixture_is_safe(self):
        fixture = Path(__file__).parent / 'Fixtures/retail_stock_advanced_graphics_open.txt'
        tick, frames = replay.parse_fixture(fixture.read_text(encoding='utf-8'))
        replay.validate_launch_safe_frames(frames)
        self.assertGreater(tick, 0)
        self.assertGreater(len(frames), 220)

    def test_probe_requires_actual_value_and_exact_setting(self):
        log = ('StockGraphicsProbe event=applied setting=frame_limit requested=120 actual=60\n'
               'StockGraphicsProbe event=applied setting=native_text requested=off actual=off\n')
        rows = smoke.probes(log)
        self.assertFalse(gpu.matching(rows, 'applied', 'frame_limit', '120'))
        self.assertTrue(gpu.matching(rows, 'applied', 'native_text', 'off'))
        self.assertFalse(gpu.matching(rows, 'created', 'native_text', 'off'))
        self.assertFalse(gpu.matching(gpu.probes('StockGraphicsGPU event=applied setting=vsync requested=off actual=on'), 'applied', 'vsync', 'off'))

    def test_change_requires_distinct_prior_actual_and_correct_order(self):
        created = dict(event='created', setting='vsync', requested='off', actual='off')
        applied = dict(event='applied', setting='vsync', requested='on', actual='on')
        self.assertTrue(smoke.changed([created, applied], 'vsync', 'on'))
        self.assertFalse(smoke.changed([applied, created], 'vsync', 'on'))
        self.assertFalse(smoke.changed([dict(created, actual='on'), applied], 'vsync', 'on'))

    def test_config_comparison_checks_numeric_and_boolean_values(self):
        self.assertTrue(smoke.config_matches('ui_scale', '1.25', '125'))
        self.assertFalse(smoke.config_matches('ui_scale', '1', '125'))
        self.assertTrue(smoke.config_matches('frame_limit', '120.0', '120'))
        self.assertFalse(smoke.config_matches('frame_limit', 'bad', '120'))
        self.assertTrue(smoke.config_matches('native_text', 'False', 'off'))
        self.assertFalse(smoke.config_matches('native_text', 'True', 'off'))
        self.assertFalse(smoke.config_matches('native_text', None, 'off'))

    def test_missing_probes_and_screenshots_cannot_pass(self):
        with tempfile.TemporaryDirectory(dir=os.environ['HP2_ARTIFACT_DIR']) as directory:
            root = Path(directory); phase = root / 'set'; phase.mkdir(); user = root / 'user'; user.mkdir()
            (phase / 'run.log').write_text('', encoding='utf-8')
            (user / 'Game.ini').write_text('[Engine.GameEngine]\nFrameRateLimit=120\n'
                '[SDLDrv.SDLClient]\nUIScale=1.25\nMaintainVerticalFOV=False\nNativeText=False\nScreenFlashes=False\n', encoding='utf-8')
            result = {'exit': 0, 'timed_out': False, 'launch_error': None, 'engine_sha256_after': 'abc',
                      'failure_markers': [], 'adapter_errors': [], 'shots': [], 'vsync': 'On', 'anisotropy': '4', 'probe': []}
            errors = smoke.check_phase(result, root, 'set', user, 'abc', [10], 'on', '4', '4')
            self.assertTrue(any('screenshots missing' in e for e in errors))
            for name in (*smoke.SETTINGS, 'vsync', 'anisotropy', 'msaa'):
                self.assertTrue(any(f'{name}' in e and 'trace' in e for e in errors), name)

if __name__ == '__main__': unittest.main()
