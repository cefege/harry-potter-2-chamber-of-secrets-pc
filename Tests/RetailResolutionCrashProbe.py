#!/usr/bin/env python3
"""Kill an isolated retail process during a 1280x720 preview and restart it."""
from __future__ import annotations
import argparse, json, os, signal, shutil, subprocess, time
from pathlib import Path
import InputScriptSmoke as replay
import RetailStockSettingsSmoke as stock
import RetailModernResolutionFixtures as fixtures
ROOT=Path(__file__).resolve().parent.parent

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--mode',choices=('windowed','fullscreen-desktop'),required=True)
    p.add_argument('--engine-bin',type=Path,required=True);p.add_argument('--sha256',required=True)
    p.add_argument('--data-root',type=Path,required=True);p.add_argument('--inventory',type=Path,required=True)
    p.add_argument('--artifact-dir',type=Path,default=os.environ.get('HP2_ARTIFACT_DIR'));p.add_argument('--allow-window-and-kill',action='store_true')
    a=p.parse_args()
    if not a.allow_window_and_kill or not a.artifact_dir:p.error('explicit window and kill acknowledgement and artifact directory required')
    data,engine,inventory=(x.resolve(strict=True) for x in (a.data_root,a.engine_bin,a.inventory)); art=a.artifact_dir.resolve()
    if art==data or art.is_relative_to(data) or data.is_relative_to(art):p.error('artifact and data trees overlap')
    art.mkdir(parents=True,exist_ok=True)
    if any(art.iterdir()):p.error('artifact directory must be empty')
    before_hash=stock.sha256(engine)
    if before_hash!=a.sha256:raise RuntimeError('binary hash mismatch')
    before=stock.identity(data,inventory);(art/'inventory-before.json').write_text(json.dumps(before,indent=2))
    if len(before)!=488 or not all(x['matches'] for x in before):raise RuntimeError('retail inventory mismatch')
    scripts=fixtures.build_fixtures((680,455),(320,178),(250,262),(550,418),(730,418),(976,546))
    prefix=scripts['timeout.txt'].split('mouse 250 262')[0]+'mouse 250 262\npress LeftMouse\nidle 60000\n'
    tick,frames=replay.parse_fixture(prefix);replay.validate_launch_safe_frames(frames)
    home=art/'home';user=replay.prepare_user_home(home,data,replay.compile_replay('PrivetDr.unr',tick,frames));ini=user/'Game.ini';text=ini.read_text()
    for sec,key,val in [('SDLDrv.SDLClient','CaptureMouse','False'),('SDLDrv.SDLClient','WindowedViewportX','800'),('SDLDrv.SDLClient','WindowedViewportY','600'),('SDLDrv.SDLClient','FullscreenViewportX','800'),('SDLDrv.SDLClient','FullscreenViewportY','600'),('SDLDrv.SDLClient','StartupFullscreen','True' if a.mode=='fullscreen-desktop' else 'False'),('Core.System','SavePath',str(user/'Save')),('Core.System','CachePath',str(user/'Cache'))]:text=stock.set_ini(text,sec,key,val)
    ini.write_text(text);(art/'preview-fixture.txt').write_text(prefix);directory=art/'preview-frames';directory.mkdir();shot=directory/'frame_000270.png'
    cmd=replay.build_launch_command(engine,data,len(frames)+80)
    if a.mode=='fullscreen-desktop':cmd.remove('-window')
    cmd.append(f'-INI="{ini}"');(art/'preview-command.json').write_text(json.dumps(cmd,indent=2))
    env=os.environ.copy();env.update(HOME=str(home),CFFIXED_USER_HOME=str(home),TMPDIR=str(home/'tmp')+'/',XDG_CONFIG_HOME=str(home/'.config'),XDG_CACHE_HOME=str(home/'.cache'),XDG_DATA_HOME=str(home/'.local/share'),LC_ALL='C',TZ='UTC',HP2_ARTIFACT_DIR=str(art),HP2_CAPTURE_FRAMES=str(directory),HP2_CAPTURE_FRAME_INDICES='220,245,270',SDL_MAC_BACKGROUND_APP='1')
    for path in (home/'tmp',home/'.config',home/'.cache'):path.mkdir(parents=True,exist_ok=True)
    killed=False;reason=None
    with (art/'preview.log').open('wb') as out:
        proc=replay.hp2_proc.popen_group(cmd,cwd=str(ROOT),env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+120
        while time.monotonic()<deadline and proc.poll() is None:
            if shot.is_file() and shot.stat().st_size>24:
                try:
                    stock.png_dimensions(shot)
                    if not shot.read_bytes().endswith(b'\x00\x00\x00\x00IEND\xaeB`\x82'):
                        time.sleep(0.005)
                        continue
                    os.killpg(proc.pid,signal.SIGKILL);killed=True
                except (OSError,ValueError):pass
                if killed:break
            time.sleep(0.005)
        if proc.poll() is None and not killed:
            reason='preview checkpoint not observed before deadline';os.killpg(proc.pid,signal.SIGTERM)
        try:status=proc.wait(timeout=8)
        except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);status=proc.wait();reason='process needed forced cleanup'
    shutil.copy2(ini,art/'Game-after-kill.ini');between=stock.sha256(engine);stem='Fullscreen' if a.mode=='fullscreen-desktop' else 'Windowed'
    def dims():return [stock.read_value(ini,'SDLDrv.SDLClient',stem+'ViewportX'),stock.read_value(ini,'SDLDrv.SDLClient',stem+'ViewportY')]
    killed_config=dims();restart=None;restart_log=''
    if killed and between==before_hash:
        script=scripts['restart-restored.txt'];tick,events=replay.parse_fixture(script);(user/'InputScriptSmoke.rep').write_bytes(replay.compile_replay('PrivetDr.unr',tick,events));(art/'restart-fixture.txt').write_text(script)
        cmd=replay.build_launch_command(engine,data,len(events)+80)
        if a.mode=='fullscreen-desktop':cmd.remove('-window')
        cmd.append(f'-INI="{ini}"');(art/'restart-command.json').write_text(json.dumps(cmd,indent=2));directory=art/'restart-frames'
        os.environ.update(HP2_ARTIFACT_DIR=str(art),HP2_CAPTURE_FRAMES=str(directory),HP2_CAPTURE_FRAME_INDICES='0,220,240',SDL_MAC_BACKGROUND_APP='1',XDG_DATA_HOME=str(home/'.local/share'))
        (out,rc,timed,error),discarded,prune=stock.supervised_checkpoints(cmd,home=home,cwd=ROOT,timeout_seconds=180,directory=directory,keep={220,240})
        (art/'restart.log').write_bytes(out);restart_log=out.decode('utf-8',errors='replace');restart={'exit':rc,'timed_out':timed,'error':error,'prune_errors':prune,'video_frame_exists':(directory/'frame_000240.png').exists()}
    shutil.copy2(ini,art/'Game-after-restart.ini');after_config=dims();after_hash=stock.sha256(engine);after=stock.identity(data,inventory);(art/'inventory-after.json').write_text(json.dumps(after,indent=2))
    preview_log=(art/'preview.log').read_text(errors='replace');problems=[]
    if not killed or status!=-signal.SIGKILL or reason:problems.append('intentional kill not established')
    if 'HPMessageBox Setup' not in preview_log:problems.append('preview confirmation not established')
    if killed_config!=['800','600'] or after_config!=['800','600']:problems.append('confirmed resolution changed')
    if restart is None or restart['exit']!=0 or restart['timed_out'] or restart['error'] or restart['prune_errors'] or not restart['video_frame_exists'] or 'ChangePage Transient.FESoundVideoPage' not in restart_log:problems.append('restart settings page not established')
    if before!=after or not before_hash==between==after_hash:problems.append('retail inventory or binary changed')
    cuts=[x for x in (preview_log+'\n'+restart_log).splitlines() if 'CutError:' in x and not x.startswith('Destroying ')]
    if cuts:problems.append('stock cutscene errors observed; not waived')
    report={'status':'fail' if problems else 'visual-review-required','mode':a.mode,'expected_kill':killed,'preview_exit':status,'kill_reason':reason,'confirmed_config_after_kill':killed_config,'confirmed_config_after_restart':after_config,'restart':restart,'binary_sha256':[before_hash,between,after_hash],'inventory_unchanged':before==after,'cut_errors':cuts,'problems':problems,'limitations':['preview and restart labels require visual review','offscreen captures do not prove desktop presentation','replay input does not prove physical input']};(art/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2));return 1 if problems else 0
if __name__=='__main__':raise SystemExit(main())
