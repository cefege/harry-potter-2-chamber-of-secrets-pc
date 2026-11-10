#!/usr/bin/env python3
"""Data-none checks for the retail popup replay's strict input evidence."""
import unittest

from RetailStockGraphicsPopupSmoke import exact_changes


def row(value, requested=None):
    return {'event': 'applied', 'setting': 'frame_limit', 'actual': value,
            'requested': value if requested is None else requested}


class PopupReplayContracts(unittest.TestCase):
    def test_mouse_then_each_keyboard_step(self):
        self.assertTrue(exact_changes([row('120'), row('144'), row('120')],
                                      'frame_limit', ['120', '144', '120']))

    def test_double_dispatch_and_ineffective_changes_fail(self):
        self.assertFalse(exact_changes([row('120'), row('144'), row('144'), row('120')],
                                       'frame_limit', ['120', '144', '120']))
        self.assertFalse(exact_changes([row('120'), row('144', '120'), row('120')],
                                       'frame_limit', ['120', '144', '120']))


if __name__ == '__main__':
    unittest.main()
