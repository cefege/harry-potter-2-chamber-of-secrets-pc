#!/usr/bin/env python3
"""Data-none preparation contracts; no engine or display."""
import unittest
import InputScriptSmoke as replay
import RetailAdvancedGraphicsFixtures as subject
import RetailGraphicsEvidenceAudit as audit

class Tests(unittest.TestCase):
    def setUp(self):
        self.cal=subject.calibration_template();self.cal['snapshot_sha256']='a'*64;self.cal['calibration_capture']='/tmp/capture.png'
        for step in self.cal['steps'].values(): step.update(set='mouse 100 100\npress LeftMouse',restart='mouse 100 100\npress LeftMouse')
        self.opening='tick 0.033333335\nidle 2\n'
    def test_all_settings_and_restart_compile(self):
        scripts,marks=subject.generate(self.cal,self.opening)
        self.assertEqual(set(scripts),{'set','restart'})
        for phase,script in scripts.items():
            tick,frames=replay.parse_fixture(script);replay.validate_launch_safe_frames(frames)
            self.assertTrue(replay.compile_replay('PrivetDr.unr',tick,frames))
            self.assertEqual(set(marks[phase]),set(subject.ORDER)|{'open'})
            self.assertEqual(list(marks[phase])[-1],'ui_scale')
            self.assertTrue(all(i<len(frames) for i in marks[phase].values()))
    def test_missing_snapshot_and_missing_input_block_generation(self):
        self.cal['snapshot_sha256']=None
        with self.assertRaises(ValueError):subject.generate(self.cal,self.opening)
        self.cal['snapshot_sha256']='a'*64;self.cal['steps']['vsync']['restart']=None
        with self.assertRaises(ValueError):subject.generate(self.cal,self.opening)
    def test_unbalanced_or_unsafe_dsl_is_rejected(self):
        self.cal['steps']['native_text']['set']='down LeftMouse'
        with self.assertRaises(replay.FixtureError):subject.generate(self.cal,self.opening)
    def test_audit_preserves_cuterrors_and_occlusion(self):
        log='**** CutError:Failed to find actor\nWgpuDrv: frames presented=0 occluded=12 outdated=0 other_failure=0\n'
        result=audit.audit({'status':'pass','inventory_unchanged':True}, {'set':log,'restart':log})
        self.assertEqual(result['status'],'fail')
        self.assertTrue(any('CutError' in line for line in result['findings']))
        self.assertTrue(any('no frames presented' in line for line in result['findings']))
        self.assertEqual(result['phases']['set']['presentation_counters'][0]['occluded'],12)

    def test_all_settings_have_distinct_effect_evidence(self):
        self.assertEqual(set(subject.EVIDENCE),set(subject.ORDER));self.assertIn('occluded',subject.EVIDENCE['vsync'])
        self.assertIn('bitmap',subject.EVIDENCE['native_text']);self.assertIn('wall-clock',subject.EVIDENCE['frame_limit'])

if __name__=='__main__': unittest.main()
