#!/usr/bin/env python3
"""One isolated retail 1280x720 resolution scenario and restart; opt-in window."""
from __future__ import annotations
import argparse, json, os, shutil
from pathlib import Path
import InputScriptSmoke as replay
import RetailStockSettingsSmoke as stock
import RetailModernResolutionFixtures as fixtures

ROOT = Path(__file__).resolve().parent.parent

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--mode', choices=['windowed','fullscreen-desktop'], required=True)
    p.add_argument('--scenario', choices=['accept','reject','timeout'], required=True)
    p.add_argument('--engine-bin', type=Path, required=True)
    p.add_argument('--sha256', required=True)
    p.add_argument('--data-root', type=Path, required=True)
    p.add_argument('--inventory', type=Path, required=True)
    p.add_argument('--artifact-dir', type=Path, default=os.environ.get('HP2_ARTIFACT_DIR'))
    p.add_argument('--allow-window', action='store_true')
    args = p.parse_args()
    if not args.allow_window or not args.artifact_dir: p.error('window acknowledgement and artifact directory are required')
    data, engine, inventory = (x.resolve(strict=True) for x in (args.data_root, args.engine_bin, args.inventory))
    artifacts = args.artifact_dir.resolve()
    if artifacts == data or artifacts.is_relative_to(data) or data.is_relative_to(artifacts): p.error('artifact and retail trees overlap')
    artifacts.mkdir(parents=True, exist_ok=True)
    if any(artifacts.iterdir()): p.error('artifact directory must be empty')
    binary_before = stock.sha256(engine)
    if binary_before != args.sha256: raise RuntimeError('snapshot hash mismatch')
    before = stock.identity(data, inventory)
    (artifacts/'inventory-before.json').write_text(json.dumps(before, indent=2))
    if len(before) != 488 or not all(x['matches'] for x in before): raise RuntimeError('retail inventory mismatch')
    scripts = fixtures.build_fixtures((680,455),(320,178),(250,262),(550,418),(730,418),(976,546))
    text = scripts[args.scenario+'.txt']
    def offset(marker):
        return len(replay.parse_fixture(text.split(marker,1)[0])[1])-1
    choice = offset('mouse 250 262')
    keep = {'popup':choice-5, 'confirmation':choice+20,
            'result':choice+(700 if args.scenario=='timeout' else 75)}
    home = artifacts/'home'
    tick, events = replay.parse_fixture(text)
    user = replay.prepare_user_home(home, data, replay.compile_replay('PrivetDr.unr', tick, events))
    ini = user/'Game.ini'
    config = ini.read_text()
    for section,key,value in [('SDLDrv.SDLClient','CaptureMouse','False'),('SDLDrv.SDLClient','WindowedViewportX','800'),('SDLDrv.SDLClient','WindowedViewportY','600'),('SDLDrv.SDLClient','FullscreenViewportX','800'),('SDLDrv.SDLClient','FullscreenViewportY','600'),('SDLDrv.SDLClient','StartupFullscreen','True' if args.mode=='fullscreen-desktop' else 'False'),('Core.System','SavePath',str(user/'Save')),('Core.System','CachePath',str(user/'Cache'))]:
        config = stock.set_ini(config, section,key,value)
    ini.write_text(config)
    os.environ.update(HP2_ARTIFACT_DIR=str(artifacts), SDL_MAC_BACKGROUND_APP='0' if args.mode=='fullscreen-desktop' else '1', XDG_DATA_HOME=str(home/'.local/share'))
    section='SDLDrv.SDLClient'; stem='Fullscreen' if args.mode=='fullscreen-desktop' else 'Windowed'
    def res(): return [stock.read_value(ini, section,stem+'ViewportX'),stock.read_value(ini,section,stem+'ViewportY')]
    def run(name, script, checkpoints):
        delta, frames = replay.parse_fixture(script); replay.validate_launch_safe_frames(frames)
        (user/'InputScriptSmoke.rep').write_bytes(replay.compile_replay('PrivetDr.unr',delta,frames))
        (artifacts/(name+'-fixture.txt')).write_text(script)
        cmd=replay.build_launch_command(engine,data,len(frames)+80)
        if args.mode=='fullscreen-desktop': cmd.remove('-window')
        cmd.append(f'-INI="{ini}"'); (artifacts/(name+'-command.json')).write_text(json.dumps(cmd,indent=2))
        directory=artifacts/(name+'-frames'); os.environ['HP2_CAPTURE_FRAMES']=str(directory)
        (output,status,timeout,error),discarded,prune=stock.supervised_checkpoints(cmd,home=home,cwd=ROOT,timeout_seconds=240,directory=directory,keep=set(checkpoints.values()))
        (artifacts/(name+'.log')).write_bytes(output); (artifacts/(name+'-discarded.json')).write_text(json.dumps(discarded))
        dims={}
        for label,index in checkpoints.items():
            try: dims[label]=stock.png_dimensions(directory/f'frame_{index:06}.png')
            except (OSError,ValueError): dims[label]=None
        log=output.decode('utf-8',errors='replace')
        return {'exit':status,'timed_out':timeout,'error':error,'prune_errors':prune,'dimensions':dims,
                'failure_markers':replay.game_test._failure_markers(output,ROOT,home),
                'key_log_lines':[x for x in log.splitlines() if not x.startswith('Destroying ') and any(s in x for s in ('CutError:','HPMessageBox','ResolutionCombo','frames presented=','SetSceneNode first call','surface is active','Render scale'))]}, log
    first,log=run('initial',text,keep); first_res=res(); binary_between=stock.sha256(engine)
    shutil.copy2(ini,artifacts/'Game-before-restart.ini')
    expected=['1280','720'] if args.scenario=='accept' else ['800','600']
    restart=None; restart_log=''
    if first['exit']==0 and first_res in (['1280','720'],['800','600']) and binary_between==binary_before:
        restart_fixture=scripts['restart-accepted.txt' if first_res==['1280','720'] else 'restart-restored.txt']
        restart,restart_log=run('restart',restart_fixture,{'video':240})
    after_res=res(); binary_after=stock.sha256(engine); after=stock.identity(data,inventory)
    (artifacts/'inventory-after.json').write_text(json.dumps(after,indent=2)); shutil.copy2(ini,artifacts/'Game-after-restart.ini')
    problems=[]
    if first['exit']!=0 or first['timed_out'] or first['error'] or first['prune_errors'] or first['failure_markers'] or None in first['dimensions'].values(): problems.append('initial process or checkpoint failed')
    if first_res != expected or after_res != expected: problems.append('resolution result or persistence mismatch')
    if 'HPMessageBox Setup' not in log: problems.append('confirmation not observed')
    if args.scenario!='timeout' and f"HPMessageBox button clicked {'Yes' if args.scenario=='accept' else 'No'}" not in log: problems.append('expected dialog button not observed')
    if args.scenario=='timeout' and 'HPMessageBox Close , Result= ' not in log: problems.append('unattended dialog close missing')
    if restart is None or restart['exit']!=0 or restart['timed_out'] or restart['error'] or restart['prune_errors'] or restart['failure_markers'] or restart['dimensions']['video'] is None or 'ChangePage Transient.FESoundVideoPage' not in restart_log: problems.append('restart video page not established')
    if not binary_before==binary_between==binary_after or before!=after: problems.append('binary or retail inventory changed')
    if any('CutError:' in x and not x.startswith('Destroying ') for x in (log+'\n'+restart_log).splitlines()): problems.append('stock cutscene errors observed; not waived')
    report={'status':'fail' if problems else 'visual-review-required','mode':args.mode,'scenario':args.scenario,'data_profile':'data-retail','binary_sha256':[binary_before,binary_between,binary_after],'inventory_count':len(before),'inventory_unchanged':before==after,'resolution_before_restart':first_res,'resolution_after_restart':after_res,'expected_resolution':expected,'initial':first,'restart':restart,'problems':problems,'limitations':['screenshots and labels require visual review','replay does not verify physical input','INI values alone do not prove live logical resolution','occluded frame captures do not prove presentation','replay time is not wall-clock time']}
    (artifacts/'report.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps(report,indent=2)); return 1 if problems else 0

if __name__=='__main__': raise SystemExit(main())
