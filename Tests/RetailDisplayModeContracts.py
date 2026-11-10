#!/usr/bin/env python3
"""Data-none contracts for retail display workflow reporting."""
import unittest
import RetailDisplayModeSmoke as smoke


class DisplayWorkflowContracts(unittest.TestCase):
    def test_no_fullscreen_when_startup_false_and_desktop_true(self):
        log = ('StockDisplayProbe startup requested=Windowed actual=Windowed startup=false desktop=true\n'
               'StockDisplayProbe created page=1 control=2 mode=Windowed\n'
               'ChangePage Transient.FESoundVideoPage0\n')
        self.assertEqual(smoke.phase_checks('initial', log, None), [])
        self.assertTrue(smoke.phase_checks('initial', log.replace('actual=Windowed', 'actual=BorderlessFullscreen'), None))
        self.assertTrue(smoke.phase_checks('initial', log, 'Windowed'))
        self.assertTrue(smoke.phase_checks('initial', log.replace('ChangePage Transient.FESoundVideoPage0', ''), None))

    def test_confirmation_and_rollback_require_saved_preference(self):
        self.assertEqual(smoke.phase_checks('keep', 'StockDisplayProbe startup requested=Windowed actual=Windowed startup=false desktop=true\nChangePage Transient.FESoundVideoPage0\nStockDisplayProbe applied mode=BorderlessFullscreen\nStockDisplayProbe confirmed', 'BorderlessFullscreen'), [])
        prefix = 'StockDisplayProbe startup requested=BorderlessFullscreen actual=BorderlessFullscreen startup=true desktop=true\nChangePage Transient.FESoundVideoPage0\nStockDisplayProbe applied mode=Windowed\n'
        for phase, marker in [('revert', 'cancelled'), ('timeout', 'dialog timeout restored')]:
            log = prefix + 'StockDisplayProbe ' + marker
            self.assertEqual(smoke.phase_checks(phase, log, 'BorderlessFullscreen'), [])
            self.assertTrue(smoke.phase_checks(phase, log, 'Windowed'))
        self.assertTrue(smoke.phase_checks('timeout', prefix + 'StockDisplayProbe cancelled', 'BorderlessFullscreen'))

    def test_exclusive_requires_accept_and_revert_or_explicit_rejection(self):
        startup = 'StockDisplayProbe startup requested=BorderlessFullscreen actual=BorderlessFullscreen startup=true desktop=true\nChangePage Transient.FESoundVideoPage0\n'
        self.assertEqual(smoke.phase_checks('exclusive', startup + 'Display mode transition failed: rejected', 'BorderlessFullscreen'), [])
        self.assertEqual(smoke.phase_checks('exclusive', startup + 'StockDisplayProbe applied mode=ExclusiveFullscreen\nStockDisplayProbe cancelled', 'BorderlessFullscreen'), [])
        self.assertTrue(smoke.phase_checks('exclusive', startup + 'StockDisplayProbe applied mode=ExclusiveFullscreen', 'BorderlessFullscreen'))
        self.assertTrue(smoke.phase_checks('exclusive', startup, 'BorderlessFullscreen'))

    def test_restart_requires_open_page_not_only_startup_mode(self):
        startup = 'StockDisplayProbe startup requested=BorderlessFullscreen actual=BorderlessFullscreen startup=true desktop=true\n'
        self.assertTrue(smoke.phase_checks('restart', startup, 'BorderlessFullscreen'))
        self.assertEqual(smoke.phase_checks('restart', startup + 'ChangePage Transient.FESoundVideoPage0', 'BorderlessFullscreen'), [])

    def test_clear_preference_changes_only_target_section(self):
        text = '[Other]\nHP2DisplayMode=keep\n[SDLDrv.SDLClient]\nHP2DisplayMode=remove\nX=1\n'
        result = smoke.remove_ini_value(text, 'SDLDrv.SDLClient', 'HP2DisplayMode')
        self.assertIn('HP2DisplayMode=keep', result)
        self.assertNotIn('HP2DisplayMode=remove', result)
        self.assertIn('X=1', result)

    def test_timeout_requires_event_free_tail_and_capture(self):
        valid = '# capture: 1\ntick 0.033333335\nidle 1\npress Enter\nidle 650\n'
        smoke.validate_fixture('timeout', valid)
        with self.assertRaises(ValueError):
            smoke.validate_fixture('timeout', valid + 'press Escape\n')
        with self.assertRaises(ValueError):
            smoke.validate_fixture('timeout', valid.replace('# capture: 1\n', ''))


if __name__ == '__main__':
    unittest.main()
