#!/usr/bin/env python3
"""Generate calibrated advanced-graphics replays without launching the game.

Calibration must come from the exact future snapshot and viewport. Replay
snippets use InputScriptSmoke's DSL; coordinates are logical, not OS input.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import InputScriptSmoke as replay

ROOT = Path(__file__).resolve().parent.parent
OPEN = ROOT/'Tests/Fixtures/retail_stock_advanced_graphics_open.txt'
# UI scale is last because a change can immediately move the other controls.
ORDER = ('frame_limit','vertical_fov','native_text','screen_flashes','vsync','anisotropy','msaa','ui_scale')
TARGETS = {'frame_limit':'120','vertical_fov':'off','native_text':'off','screen_flashes':'off',
           'vsync':'on','anisotropy':'4','msaa':'4','ui_scale':'125'}
EVIDENCE = {
 'frame_limit':'Compare wall-clock frame pacing under identical uncapped and capped conditions; fixed-delta replay is not timing evidence.',
 'vertical_fov':'Capture the same 3D scene at a non-4:3 aspect ratio and compare projection/FOV telemetry.',
 'native_text':'Capture representative text and verify the live native/bitmap path; preserve NativeText=False bitmap compatibility.',
 'screen_flashes':'Use a deterministic verified flash-producing event; compare on/off imagery and event trace.',
 'vsync':'Verify actual GPU present-mode readback; a locked or occluded display cannot establish visible synchronization.',
 'anisotropy':'Verify effective GPU sampler state and inspect a matched oblique texture scene.',
 'msaa':'Verify effective sample count and inspect matched geometry edges; report unsupported sample counts.',
 'ui_scale':'Compare stable UI geometry at 100% and 125%; inspect layout, clipping and physical hit alignment separately.',
}

def calibration_template():
    return {'snapshot_sha256':None,'calibration_capture':None,
            'steps':{name:{'set':None,'restart':None} for name in ORDER}}


def generate(calibration, opening):
    digest=calibration.get('snapshot_sha256')
    if not isinstance(digest,str) or len(digest)!=64 or any(c not in '0123456789abcdef' for c in digest):
        raise ValueError('calibration requires a lowercase 64-character snapshot SHA-256')
    if not isinstance(calibration.get('calibration_capture'),str) or not calibration['calibration_capture'].strip():
        raise ValueError('calibration capture path is required')
    steps=calibration.get('steps')
    if not isinstance(steps,dict) or set(steps)!=set(ORDER): raise ValueError('calibration must contain all 8 settings')
    tick,frames=replay.parse_fixture(opening); replay.validate_launch_safe_frames(frames)
    scripts={}; checkpoints={}
    for phase in ('set','restart'):
        script=opening.rstrip()+'\n'; marks={'open':len(frames)-1}
        for name in ORDER:
            step=steps[name]
            part=step.get(phase) if isinstance(step,dict) else None
            if not isinstance(part,str) or not part.strip(): raise ValueError(f'{name}: missing {phase} input snippet')
            if any(line.strip().startswith('tick ') for line in part.splitlines()): raise ValueError('snippets must not contain tick')
            script+='\n# '+phase+' '+name+'\n'+part.rstrip()+'\nidle 20\n'
            _,events=replay.parse_fixture(script); replay.validate_launch_safe_frames(events)
            marks[name]=len(events)-5
        replay.compile_replay('PrivetDr.unr',tick,events)
        scripts[phase]=script; checkpoints[phase]=marks
    return scripts,checkpoints


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output-dir',type=Path,required=True,help='new directory, preferably under HP2_ARTIFACT_DIR')
    p.add_argument('--calibration',type=Path,help='snapshot-specific JSON with set and restart DSL snippets')
    args=p.parse_args()
    if args.calibration is None:
        args.output_dir.mkdir(parents=True,exist_ok=False)
        (args.output_dir/'calibration-template.json').write_text(json.dumps(calibration_template(),indent=2)+'\n')
        print('Template created; calibration and replay remain blocked. No game launched.');return 0
    cal=json.loads(args.calibration.read_text()); scripts,checkpoints=generate(cal,OPEN.read_text())
    args.output_dir.mkdir(parents=True,exist_ok=False)
    for phase,text in scripts.items(): (args.output_dir/f'{phase}.txt').write_text(text)
    plan={'status':'prepared-not-run','calibration_snapshot_sha256':cal['snapshot_sha256'],
          'calibration_capture':cal['calibration_capture'],'setting_order':ORDER,'expected_targets':TARGETS,
          'checkpoints':checkpoints,'live_effect_evidence_required':EVIDENCE,
          'conditions':['Display locked: do not launch until released and snapshot is stable.',
           'Use isolated HOME, saves, INI, TMPDIR, and an inventory-verified disposable retail copy.',
           'Check binary SHA-256 before/between/after and inventory before/after.',
           'Report CutError, missing probes, unsupported settings and occluded frames; never waive them.',
           'Retain both runs, logs, checkpoint images and restart state; review every image.',
           'Actual-value probes and INI persistence do not by themselves establish visual or physical input effects.',
           'No retail authenticity claim follows from the inventory.']}
    (args.output_dir/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')
    print(f'Prepared 2 fixtures in {args.output_dir}; no game launched.')
    return 0

if __name__=='__main__': raise SystemExit(main())
