#!/usr/bin/env python3
"""Deterministically inventory stock HP2 spell interactions from UE1 packages."""
from __future__ import annotations
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import struct
import sys
from typing import NoReturn, Sequence

if __package__:
    from . import package79_reference as p79
    from . import smoke_maps
else:
    import package79_reference as p79
    import smoke_maps

OUTPUT = Path("Tests/Fixtures/spell-interactions.json")
DATA_ROOT = Path("HarryPotter2/Unreal")
HAS_STACK = 0x02000000
SPELLS = ("None","Alohomora","Incendio","LocomotorWibbly","Lumos","Nox",
 "PetrificusTotalus","WingardiumLeviosa","Verdimillious","Vermillious","Flintifores",
 "Reparo","MucorAdNauseum","Flipendo","Ectomatic","Avifores","FireCracker",
 "Transfiguration","WingSustain","Diffindo","Skurge","Spongify","Rictusempra",
 "Ecto","Fire","DuelRictusempra","DuelMimblewimble","DuelExpelliarmus")
CASTABLE = frozenset((1,4,13,19,20,21,22,25,26,27))
DEFAULT_BOOK = frozenset((1,4,13))
MOVER_STATES = frozenset(("TriggerOpenTimed","TriggerToggle","TriggerControl","TriggerPound","LoopMove"))
LESSONS = {0:22,1:20,2:19,3:21}

class AuditError(Exception): pass
def fail(message: str) -> NoReturn: raise AuditError(message)

def array_index(c: p79.Cursor) -> int:
    a=c.u8(); value=a&127
    if a&128:
        b=c.u8(); value|=(b&127)<<7
        if b&128: value|=(c.u8()&127)<<14; value|=c.u8()<<21
    return value

def tags(payload: bytes, names: list[dict[str,object]], rel: str, context: str,
         start: int=0) -> tuple[list[dict[str,object]],int]:
    """Detailed form of smoke_maps._tagged_properties, with identical wire rules."""
    c=p79.Cursor(payload,rel,start,context); out=[]
    while True:
        off=c.pos; ni=c.compact_index()
        if not 0<=ni<len(names): fail(f"{rel}: offset {off}: invalid {context} property name {ni}")
        name=str(names[ni]["text"])
        if name=="None": return out,c.pos
        info=c.u8(); kind=info&15
        if not 1<=kind<=13: fail(f"{rel}: offset {off}: invalid {context}.{name} type {kind}")
        struct_name=None
        if kind==10:
            si=c.compact_index()
            if not 0<=si<len(names): fail(f"{rel}: offset {off}: invalid {context}.{name} struct")
            struct_name=str(names[si]["text"])
        code=info&0x70
        if code in (0,16,32,48,64): size={0:1,16:2,32:4,48:12,64:16}[code]
        elif code==80: size=c.u8()
        elif code==96: size=c.u16()
        else:
            size=c.i32()
            if size<0: fail(f"{rel}: offset {off}: negative {context}.{name} size")
        boolean=bool(info&128) if kind==3 else None
        index=array_index(c) if info&128 and kind!=3 else 0
        out.append({"name":name,"kind":kind,"raw":c.take(size),"bool":boolean,
                    "index":index,"struct":struct_name})

def ref_path(package: dict[str,object], ref: int) -> str|None:
    if ref==0: return None
    table=package["exports"] if ref>0 else package["imports"]
    index=ref-1 if ref>0 else -ref-1
    if not 0<=index<len(table): fail(f"object reference {ref} is out of range")
    value=table[index]["object_path"]
    return None if value is None else str(value)

def decode(item: dict[str,object], package: dict[str,object], rel: str
           ) -> tuple[tuple[str,int],object]:
    name=str(item["name"]); kind=int(item["kind"]); raw=item["raw"]
    if not isinstance(raw,bytes): fail(f"{rel}: {name} payload is not bytes")
    if kind==1: value=int.from_bytes(raw,"little")
    elif kind==2 and len(raw)==4: value=struct.unpack("<i",raw)[0]
    elif kind==3: value=bool(item["bool"])
    elif kind==4 and len(raw)==4:
        value=struct.unpack("<f",raw)[0]
        if not math.isfinite(value): fail(f"{rel}: {name} is non-finite")
    elif kind in (5,8):
        c=p79.Cursor(raw,rel,0,name); value=ref_path(package,c.compact_index())
        if c.pos!=len(raw): fail(f"{rel}: {name} object reference has trailing bytes")
    elif kind==6:
        c=p79.Cursor(raw,rel,0,name); ni=c.compact_index()
        if not 0<=ni<len(package["names"]) or c.pos!=len(raw): fail(f"{rel}: invalid {name} FName")
        value=str(package["names"][ni]["text"])
    elif kind==10 and len(raw)==12 and str(item["struct"]).casefold()=="vector":
        value=list(struct.unpack("<fff",raw))
    elif kind==10 and len(raw)==12 and str(item["struct"]).casefold()=="rotator":
        value=list(struct.unpack("<iii",raw))
    elif kind==13: value=smoke_maps._decode_fstring(raw,rel,name)
    else: value={"raw_hex":raw.hex(),"type":kind,"struct":item["struct"]}
    return (name,int(item["index"])),value

def active_exports(data: bytes, package: dict[str,object], rel: str) -> list[dict[str,object]]:
    levels=[e for e in package["exports"] if e["class_path"]=="Engine.Level"]
    if len(levels)!=1: fail(f"{rel}: expected one Level, found {len(levels)}")
    serial=levels[0]["serial"]
    if serial is None: fail(f"{rel}: Level has no payload")
    payload=data[serial["offset"]:serial["end"]]
    _,end=smoke_maps._tagged_properties(payload,package["names"],rel,"Level")
    c=p79.Cursor(payload,rel,end,"Level actor array"); count=c.i32(); capacity=c.i32()
    if count<0 or capacity<count: fail(f"{rel}: invalid actor array {count}/{capacity}")
    out=[]
    for slot in range(count):
        ref=c.compact_index()
        if ref==0: continue
        if ref<0 or ref>len(package["exports"]): fail(f"{rel}: actor slot {slot} has bad ref {ref}")
        export=package["exports"][ref-1]
        if export["serial"] is None: fail(f"{rel}: active actor {export['object_path']} has no payload")
        out.append(export)
    return out

def overrides(data: bytes, package: dict[str,object], rel: str, export: dict[str,object]
             ) -> dict[tuple[str,int],object]:
    serial=export["serial"]; payload=data[serial["offset"]:serial["end"]]
    c=p79.Cursor(payload,rel,0,str(export["object_path"]))
    if int(export["object_flags"])&HAS_STACK:
        node=c.compact_index(); c.compact_index(); c.take(8); c.i32()
        if node: c.compact_index()
    found,end=tags(payload,package["names"],rel,str(export["object_path"]),c.pos)
    if end!=len(payload): fail(f"{rel}: {export['object_path']} has trailing custom data")
    return dict(decode(item,package,rel) for item in found)

class Catalog:
    def __init__(self,data_root: Path):
        self.entries={}; self.supers={}; self.cache={}; self.effective={}
        for filename in ("Core.u","Engine.u","HGame.u"):
            rel=f"System/{filename}"; data,package=smoke_maps._read_package(data_root/rel,rel)
            for export in package["exports"]:
                if export["class_path"]=="Core.Class":
                    path=str(export["object_path"]); self.entries[path]=(data,package,rel,export)
                    self.supers[path]=None if export["super_path"] is None else str(export["super_path"])
    def isa(self,path: str,root: str) -> bool:
        seen=set()
        while path and path not in seen:
            if path==root: return True
            seen.add(path); path=self.supers.get(path)
        return False
    def defaults(self,path: str) -> dict[tuple[str,int],object]:
        if path in self.cache: return self.cache[path]
        if path not in self.entries: self.cache[path]={}; return {}
        data,package,rel,export=self.entries[path]; serial=export["serial"]
        payload=data[serial["offset"]:serial["end"]]; names=package["names"]
        imports=package["imports"]; exports=package["exports"]; candidates=[]
        for start in range(max(0,len(payload)-41)):
            try:
                c=p79.Cursor(payload,rel,start,f"{path} metadata"); c.take(16)
                if c.u16()!=65535 or c.u32()!=0: continue
                c.u32(); c.take(16); count=c.compact_index()
                if not 0<=count<=4096: continue
                valid=True
                for _ in range(count):
                    ref=c.compact_index(); deep=c.i32(); c.u32()
                    if not -len(imports)<=ref<=len(exports) or deep not in (0,1): valid=False; break
                if not valid: continue
                count=c.compact_index()
                if not 0<=count<=len(names): continue
                for _ in range(count):
                    if not 0<=c.compact_index()<len(names): valid=False; break
                if not valid: continue
                within=c.compact_index(); config=c.compact_index()
                if not -len(imports)<=within<=len(exports) or not 0<=config<len(names): continue
                props,end=tags(payload,names,rel,f"{path} defaults",c.pos)
                if end==len(payload): candidates.append(props)
            except (AuditError,p79.PackageFormatError,ValueError,IndexError,KeyError): pass
        if len(candidates)!=1: fail(f"{rel}: class {path} has {len(candidates)} default suffixes, expected one")
        result=dict(decode(item,package,rel) for item in candidates[0]); self.cache[path]=result
        return result
    def properties(self,path: str) -> dict[tuple[str,int],object]:
        if path in self.effective: return dict(self.effective[path])
        chain=[]; seen=set(); current=path
        while current and current not in seen:
            chain.append(current); seen.add(current); current=self.supers.get(current)
        result={}
        for item in reversed(chain): result.update(self.defaults(item))
        self.effective[path]=dict(result); return result

class Actor:
    def __init__(self,export: dict[str,object],props: dict[tuple[str,int],object],catalog: Catalog):
        self.object=str(export["object_path"]); self.cls=str(export["class_path"]); self.p=props; self.catalog=catalog
    def get(self,name: str,index: int=0,default: object=None) -> object: return self.p.get((name,index),default)
    def isa(self,root: str) -> bool: return self.catalog.isa(self.cls,root)

def clean(value: object) -> str|None:
    return value if isinstance(value,str) and value and value.casefold()!="none" else None
def spell_name(spell: int|None) -> str|None:
    if spell is None: return None
    return SPELLS[spell] if 0<=spell<len(SPELLS) else f"Unknown({spell})"
def collision(actor: Actor,aimed: bool,impact: bool) -> dict[str,object]:
    return {"b_collide_actors":bool(actor.get("bCollideActors",default=False)),
     "b_block_actors":bool(actor.get("bBlockActors",default=False)),
     "b_block_players":bool(actor.get("bBlockPlayers",default=False)),
     "b_proj_target":bool(actor.get("bProjTarget",default=False)),
     "reticle_requires_proj_target":aimed,"spell_impact_requires_collide_actors":impact}
def state(actor: Actor) -> object:
    initial=clean(actor.get("InitialState"))
    if actor.isa("Engine.Mover"):
        return {"initial_state":initial,"key_num":int(actor.get("KeyNum",default=0)),
                "num_keys":int(actor.get("NumKeys",default=2))}
    return initial
def move(actor: Actor|None) -> object:
    if actor is None or not actor.isa("Engine.Mover"): return None
    if actor.isa("Engine.GridMover"):
        return {"dynamic_axis":"spell-hit direction","move_increment":float(actor.get("MoveIncrement",default=64.0))}
    source=int(actor.get("KeyNum",default=0)); count=max(1,int(actor.get("NumKeys",default=2)))
    target=0 if clean(actor.get("InitialState"))=="TriggerToggle" and source else count-1
    a=actor.get("KeyPos",source,[0.0,0.0,0.0]); b=actor.get("KeyPos",target,[0.0,0.0,0.0])
    if not isinstance(a,list) or not isinstance(b,list): fail(f"{actor.object}: invalid KeyPos")
    return {"from_key":source,"to_key":target,"translation":[float(b[i])-float(a[i]) for i in range(3)]}
def target_kind(actor: Actor) -> str:
    text=f"{actor.object} {actor.cls} {actor.get('Tag',default='')}".casefold()
    if actor.isa("Engine.Dispatcher"): return "dispatcher"
    if actor.cls=="Engine.RoundRobin": return "round_robin"
    if actor.isa("Engine.Mover"):
        if "door" in text:return "door"
        if "wall" in text or "secret" in text:return "wall"
        if "ledge" in text:return "ledge"
        if "block" in text or "luggage" in text:return "block"
        return "mover"
    return "actor"
def rec(map_name: str,source: Actor,mechanism: str,spell: int|None,event: str|None,
        target: Actor|None,status: str,reason: str,contract: dict[str,object]) -> dict[str,object]:
    return {"map":map_name,"source_object":source.object,"source_class":source.cls,
     "mechanism":mechanism,"spell_enum":spell,"spell_name":spell_name(spell),"event":event,
     "target_object":target.object if target else None,"target_class":target.cls if target else None,
     "target_state":state(target) if target else None,"key_displacement":move(target),
     "collision_contract":contract,"status":status,"reason":reason}
def availability(spell: int) -> tuple[str,str]:
    if spell not in CASTABLE:return "mismatched_spell",f"baseWand has no class mapping for {spell_name(spell)}"
    if spell in DEFAULT_BOOK:return "valid",f"{spell_name(spell)} is in Harry's default spell book"
    return "runtime_contract_required",f"{spell_name(spell)} requires lesson/duel flow or bNoSpellBookCheck"
def direct_mechanism(actor: Actor) -> str:
    if actor.isa("HGame.chestbronze"):return "chest.HandleSpellAlohomora"
    if actor.isa("HGame.gargoyle"):return "gargoyle.HandleSpellLumos"
    if actor.isa("Engine.GridMover"):return "mover.GridMover.spell-bump"
    text=f"{actor.object} {actor.cls}".casefold()
    if actor.isa("HGame.HDiffindo") or "vase" in text:return "breakable.HandleSpell"
    if actor.isa("HGame.HAlohomora") or "door" in text or "padlock" in text:return "door.HandleSpellAlohomora"
    return "direct.HPawn.HandleSpell" if actor.isa("HGame.HPawn") else "direct.Actor.spell-contract"

def emit_chain(records: list[dict[str,object]],map_name: str,source: Actor,mechanism: str,
 spell: int,event: str|None,by_tag: dict[str,list[Actor]],status: str,reason: str,
 contract: dict[str,object],path: frozenset[str]=frozenset()) -> None:
    targets=[] if event is None else by_tag.get(event.casefold(),[])
    if not targets:
        why=reason+("; Event is empty" if event is None else f"; no active actor has Tag {event}")
        records.append(rec(map_name,source,mechanism,spell,event,None,"unresolved_target",why,contract)); return
    for target in targets:
        why=reason
        if len(targets)>1:why+=f"; Event resolves to {len(targets)} same-Tag actors, duplicate retained"
        edge=status; kind=target_kind(target)
        if target.isa("Engine.Mover"):
            initial=clean(target.get("InitialState"))
            if initial not in MOVER_STATES:edge="inert_transition";why+=f"; mover state {initial or 'None'} has no Trigger transition"
            else:why+=f"; mover state {initial} changes key"
        records.append(rec(map_name,source,f"{mechanism}:{kind}",spell,event,target,edge,why,contract))
        if kind not in ("dispatcher","round_robin") or target.object in path:continue
        for index in range(16):
            output=clean(target.get("OutEvents",index))
            if output is None:continue
            rr=kind=="round_robin"
            emit_chain(records,map_name,target,"spell-driven-dispatcher.OutEvents->Tag",spell,output,
             by_tag,"runtime_contract_required" if rr else edge,
             f"OutEvents[{index}] "+("depends on activation order" if rr else "dispatches in order"),
             collision(target,False,False),path|{target.object})

def audit_map(path: Path,rel: str,catalog: Catalog) -> tuple[dict[str,object],list[dict[str,object]]]:
    data,package=smoke_maps._read_package(path,rel); actors=[]
    for export in active_exports(data,package,rel):
        props=catalog.properties(str(export["class_path"]));props.update(overrides(data,package,rel,export))
        actors.append(Actor(export,props,catalog))
    by_tag={}
    for actor in actors:
        tag=clean(actor.get("Tag"))
        if tag:by_tag.setdefault(tag.casefold(),[]).append(actor)
    for values in by_tag.values():values.sort(key=lambda a:a.object.encode())
    records=[]
    for actor in actors:
        raw=actor.get("eVulnerableToSpell",default=0)
        if not isinstance(raw,int):fail(f"{rel}: {actor.object} spell enum is not integer")
        spell=raw
        if actor.isa("HGame.spellTrigger"):
            event=clean(actor.get("Event"));contract=collision(actor,True,True)
            if not 0<spell<len(SPELLS) or spell not in CASTABLE:status="mismatched_spell";why=f"spell enum {spell} has no castable class"
            elif not bool(actor.get("bInitiallyActive",default=True)):status="runtime_contract_required";why="requires upstream activation; matching spell is rejected while inactive"
            else:status="valid";why="configured spell forwards Event to matching Tag"
            emit_chain(records,rel,actor,"spellTrigger.Event->Tag",spell,event,by_tag,status,why,contract)
            aim=bool(actor.get("bProjTarget",default=False)) and bool(actor.get("bCollideActors",default=False)) and bool(actor.get("bInitiallyActive",default=True))
            records.append(rec(rel,actor,"aim-reticle.activation",spell,event,actor,"valid" if aim else "runtime_contract_required",
             "reticle requires effective bProjTarget, bCollideActors, and activation",contract))
            gate,why=availability(spell);records.append(rec(rel,actor,"spell-availability.gate",spell,None,actor,gate,why,collision(actor,False,False)));continue
        if actor.isa("HGame.LumosTrigger"):
            spell=4;event=clean(actor.get("Event"));enter=bool(actor.get("bEventEntering",default=True));leave=bool(actor.get("bEventLeaving",default=False))
            emit_chain(records,rel,actor,"LumosLight->LumosTrigger.Event->Tag",spell,event,by_tag,
             "valid" if enter or leave else "inert_transition","LumosLight proximity forwards configured Event",
             collision(actor,False,False))
            if enter and leave:emit_chain(records,rel,actor,"LumosTrigger.leaving-transition",spell,event,by_tag,
             "inert_transition","entering sets bFirstEventSent so leaving cannot fire; radius guard is commented out",collision(actor,False,False))
            gate,why=availability(spell);records.append(rec(rel,actor,"spell-availability.gate",spell,None,actor,gate,why,collision(actor,False,False)));continue
        if actor.cls=="HGame.SpellLessonTrigger":
            learned=LESSONS.get(actor.get("LessonShape",default=0))
            records.append(rec(rel,actor,"spell-availability.lesson",learned,clean(actor.get("Event")),actor,
             "valid" if learned is not None else "malformed_data",f"completed lesson adds {spell_name(learned)}",collision(actor,False,False)))
        if actor.cls=="HGame.TriggerTurnOnAllSpells":records.append(rec(rel,actor,"spell-availability.bypass",None,clean(actor.get("Event")),actor,
             "runtime_contract_required","Trigger enables bNoSpellBookCheck for this level",collision(actor,False,False)))
        if spell==0:continue
        event=clean(actor.get("Event"));contract=collision(actor,True,True);mechanism=direct_mechanism(actor)
        if spell not in CASTABLE:status="mismatched_spell";why=f"baseWand has no class mapping for {spell_name(spell)}"
        elif actor.isa("Engine.GridMover"):
            status="valid" if clean(actor.get("InitialState"))=="BumpMove" else "inert_transition";why="GridMover requires BumpMove and derives displacement from hit direction"
        elif actor.isa("HGame.chestbronze") and spell==1:status="valid";why="waitforspell handles Alohomora and opens the chest"
        elif actor.isa("HGame.gargoyle") and spell==4:status="valid";why="HandleSpellLumos turns on Lumos and enters green state"
        elif actor.isa("HGame.HDiffindo") and spell==19:status="valid";why="inherited HDiffindo handler accepts Diffindo"
        elif actor.isa("HGame.HAlohomora") and spell==1:status="valid";why="inherited HAlohomora handler accepts Alohomora"
        elif actor.isa("HGame.HPawn"):status="runtime_contract_required";why=f"runtime/state handler must accept {spell_name(spell)}"
        else:status="mismatched_spell";why=f"{actor.cls} has no package-level spellTrigger/GridMover/HPawn contract"
        records.append(rec(rel,actor,mechanism,spell,event,actor,status,why,contract))
        aim=bool(actor.get("bProjTarget",default=False)) and bool(actor.get("bCollideActors",default=False))
        records.append(rec(rel,actor,"aim-reticle.activation",spell,event,actor,"valid" if aim else "runtime_contract_required",
         "reticle and impact require effective bProjTarget and bCollideActors",contract))
        gate,gwhy=availability(spell);records.append(rec(rel,actor,"spell-availability.gate",spell,None,actor,gate,gwhy,collision(actor,False,False)))
        if event:
            explicit_dispatch = (
                (actor.isa("HGame.Ectoplasma") and spell == 20)
                or (actor.isa("HGame.HAlohomora") and spell == 1)
                or (actor.isa("HGame.HDiffindo") and spell == 19)
            )
            dispatch = bool(actor.get("bSpellCausesTrigger", default=False)) or explicit_dispatch
            emit_chain(records,rel,actor,"spell-driven-HPawn.Event->Tag",spell,event,by_tag,
             status if dispatch else "inert_transition",why+("; successful hit dispatches Event" if dispatch else "; bSpellCausesTrigger is false"),contract)
    records.sort(key=sort_key)
    return {"map":rel,"file_sha256":package["file_sha256"],"package_version":package["summary"]["version"],
            "package_licensee":package["summary"]["licensee"],"interaction_count":len(records)},records

def sort_key(r: dict[str,object]) -> tuple[bytes,...]:
    fields=(r["map"],r["source_object"],r["mechanism"],-1 if r["spell_enum"] is None else r["spell_enum"],r["event"] or "",r["target_object"] or "",r["status"],r["reason"])
    return tuple(str(v).encode() for v in fields)
def generate(repo: Path,data: Path) -> dict[str,object]:
    catalog=Catalog(data);maps=[];records=[]
    for path,rel in smoke_maps._enumerate_maps(data):entry,found=audit_map(path,rel,catalog);maps.append(entry);records.extend(found)
    records.sort(key=sort_key); mechanisms=Counter(str(r["mechanism"]) for r in records);statuses=Counter(str(r["status"]) for r in records);spells=Counter((r["spell_enum"],r["spell_name"]) for r in records)
    try:display=data.relative_to(repo).as_posix()
    except ValueError:display=str(data)
    summary={"map_count":len(maps),"interaction_count":len(records),
     "by_mechanism":[{"mechanism":k,"count":mechanisms[k]} for k in sorted(mechanisms)],
     "by_spell":[{"spell_enum":k[0],"spell_name":k[1],"count":v} for k,v in sorted(spells.items(),key=lambda x:-1 if x[0][0] is None else x[0][0])],
     "by_status":[{"status":k,"count":statuses[k]} for k in sorted(statuses)]}
    return {"format":"hp2-stock-spell-interactions","schema_version":1,"data_root":display,"maps":maps,"records":records,"summary":summary}
def encoded(report: dict[str,object]) -> bytes:return (json.dumps(report,ensure_ascii=False,indent=2,sort_keys=True)+"\n").encode()
def arguments(argv: Sequence[str]|None=None) -> argparse.Namespace:
    root=Path(__file__).resolve().parent.parent;parser=argparse.ArgumentParser(description="Audit stock HP2 package spell interactions")
    parser.add_argument("--repo-root",type=Path,default=root);parser.add_argument("--data-root",type=Path,default=DATA_ROOT);parser.add_argument("--output",type=Path,default=OUTPUT);parser.add_argument("--check",action="store_true",help="verify byte-for-byte regeneration");return parser.parse_args(argv)
def resolve(value: Path,root: Path,strict: bool) -> Path:
    if not value.is_absolute():value=root/value
    try:return value.expanduser().resolve(strict=strict)
    except OSError as error:raise AuditError(f"cannot resolve {value}: {error}") from error
def main(argv: Sequence[str]|None=None) -> int:
    a=arguments(argv)
    try:
        repo=resolve(a.repo_root,Path.cwd(),True);data=resolve(a.data_root,repo,True);output=resolve(a.output,repo,False);contents=encoded(generate(repo,data))
        if a.check:
            if output.read_bytes()!=contents:fail(f"{output}: fixture differs; regenerate without --check")
            print(f"verified {output} ({len(contents)} bytes)")
        else:output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(contents);print(f"wrote {output} ({len(contents)} bytes)")
        print(json.dumps(json.loads(contents)["summary"],sort_keys=True,separators=(",",":")))
    except (AuditError,p79.PackageFormatError,OSError,UnicodeError,ValueError,IndexError,KeyError) as error:print(f"spell_interaction_audit.py: error: {error}",file=sys.stderr);return 1
    return 0
if __name__=="__main__":raise SystemExit(main())
