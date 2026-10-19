#!/bin/sh
# Regenerate the pawn-native files oracle fixture from original UnPawn.cpp at 32dcfda.
# Usage: gen_pawn_native_files.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=32dcfda
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
SRC="$WORK/HarryPotter2/Unreal/Engine/Src"
cp "$REPO/Build/pawn_native_files_fixture_harness/PawnFilesHooks.h" "$SRC/"
python3 - "$WORK/HarryPotter2/Unreal/Core/Inc/UnScript.h" <<'PY'
import pathlib, re, sys
# Harness-only: FFrame::Step is routed to the harness argument queue, and the
# one-argument FFrame constructor points Code at a zeroed harness buffer.
p = pathlib.Path(sys.argv[1])
s = p.read_bytes().decode('utf-8').replace('\r\n', '\n')
step_old = ('inline void FFrame::Step( UObject* Context, RESULT_DECL )\n{\n'
    '\tguardSlow(FFrame::Step);\n\tINT B = *Code++;\n'
    '\t(Context->*GNatives[B])( *this, Result );\n'
    '\tunguardfSlow(( TEXT("(%s @ %s : %04X)"), Object->GetFullName(), Node->GetFullName(), Code - &Node->Script(0) ));\n}\n')
assert s.count(step_old) == 1
s = s.replace(step_old, 'void PawnFilesStepArg( UObject* Context, void* const Result );\n'
    'inline void FFrame::Step( UObject* Context, RESULT_DECL )\n{\n\tPawnFilesStepArg( Context, Result );\n}\n', 1)
ctor = 'inline FFrame::FFrame( UObject* InObject )\n'
assert s.count(ctor) == 1
s = s.replace(ctor, 'BYTE* PawnFilesHarnessCode();\n' + ctor, 1)
s, n = re.subn(r',(\s*)Code(\s+)\(NULL\)', lambda m: ',' + m.group(1) + 'Code' + m.group(2) + '(PawnFilesHarnessCode())', s)
assert n == 1
p.write_bytes(s.encode('utf-8'))
PY
python3 - "$SRC/UnPawn.cpp" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
s = p.read_bytes().decode('utf-8').replace('\r\n', '\n')
anchor = '#include "FConfigCacheIni.h"'
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + '\n#include "PawnFilesHooks.h"', 1)

# The 13 natives owned by the files slice. Each span runs from the signature to the
# column-0 closing brace of the function. Rewrites are applied only inside these spans.
names = ['GetNextIntDesc', 'GetNextInt', 'GetCacheEntry', 'MoveCacheEntry',
         'CheckValidSkinPackage', 'UpdateURL', 'GetDefaultURL', 'GetNextSkin',
         'ResetKeyboard', 'FindBestInventoryPath', 'GetMapName', 'ConsoleCommand',
         'ScreenToWorld', 'GetURLMap', 'GetEntryLevel']
span_re = re.compile(r'^void \w+::exec(?:' + '|'.join(names) + r')\(.*?^\}\n', re.S | re.M)

rules = [
    ('*(FString*)Result = ((UGameEngine*)GetLevel()->Engine)->LastURL.Map;', '*(FString*)Result = PawnFilesLastURLMap();', 1),
    ('\tcheck(XLevel);\n\tcheck(XLevel->Engine);\n\tcheck((UGameEngine*)(XLevel->Engine));\n\tcheck(((UGameEngine*)(XLevel->Engine))->GEntry);\n\n\t*(ALevelInfo**)Result = ((UGameEngine*)(XLevel->Engine))->GEntry->GetLevelInfo();',
     '\tPawnFilesEntryLevelResult( this, Result );', 1),
    ('FindObjectChecked<UClass>( ANY_PACKAGE, *ClassName )', 'PawnFilesFindObjectChecked( *ClassName )', 2),
    ('GetRegistryObjects( List, UClass::StaticClass(), TempClass, 0 )', 'PawnFilesRegistryObjects( List, TempClass )', 2),
    ('GetRegistryObjects( List, UTexture::StaticClass(), NULL, 0 )', 'PawnFilesRegistryObjects( List, PawnFilesTextureClass() )', 1),
    ('FConfigCacheIni CacheIni;', '', 2),
    ('CacheIni.GetSectionPrivate(', 'PawnFilesCacheSection(', 1),
    ('CacheIni.GetString(', 'PawnFilesCacheGetString(', 1),
    ('CacheIni.SetString(', 'PawnFilesCacheSetString(', 1),
    ('debugf( TEXT("MoveCacheEntry', 'PawnFilesDebugf( TEXT("MoveCacheEntry', 1),
    ('debugf(TEXT("MoveCacheEntry', 'PawnFilesDebugf(TEXT("MoveCacheEntry', 1),
    ('GFileManager->Move(', 'PawnFilesMove(', 1),
    ('GFileManager->FindFiles(', 'PawnFilesFindFiles(', 1),
    (r'FString::Printf(TEXT("\\"))', r'FString(TEXT("\\"))', 1),
    ('*FURL::DefaultMapExt', '*PawnFilesMapExt()', 1),
    ('FindObject<UClass>( ANY_PACKAGE, ClassName)', 'PawnFilesFindObject( ClassName )', 1),
    ('FindField<UProperty>( Class, PropertyName)', 'PawnFilesFindProperty( Class, PropertyName )', 1),
    ('Class->Defaults.Num()', 'PawnFilesDefaultsNum( Class )', 1),
    ('Property->ExportText( 0, Temp, &Class->Defaults(0), &Class->Defaults(0), PPF_Localized );',
     'PawnFilesExportText( Property, Temp );', 1),
    ('for ( INT i=0; i<8; i++ )', 'for ( INT i=0; i<7; i++ )', 1),
    ('CastChecked<UGameEngine>( GetLevel()->Engine )', '((UGameEngine*)GetLevel()->Engine)', 1),
    ('GameEngine->LastURL.AddOption(', 'PawnFilesURLAddOption( GameEngine->LastURL, ', 1),
    ('GameEngine->LastURL.SaveURLConfig(', 'PawnFilesURLSaveConfig( GameEngine->LastURL, ', 1),
    ('URL.LoadURLConfig(', 'PawnFilesURLLoadConfig( URL, ', 1),
    ('URL.GetOption(', 'PawnFilesURLGetOption( URL, ', 1),
    ('Cast<UViewport>(Player)', 'PawnFilesCastViewport(Player)', 1),
    ('ResetConfig(Viewport->Input->GetClass())', 'PawnFilesResetConfig(Viewport->Input->GetClass())', 1),
    ('findPathTowardBestInventory(newPath, 1, *Weight, bPredictRespawns)',
     'PawnFilesFindPathTowardBest(this, newPath, 1, *Weight, bPredictRespawns)', 1),
    ('bestPath->IsProbing(NAME_SpecialHandling)', 'PawnFilesIsProbing(bestPath, NAME_SpecialHandling)', 1),
    ('HandleSpecial(bestPath);', 'PawnFilesHandleSpecial(this, bestPath);', 1),
    ('GetLevel()->Engine->Client->Viewports(0)', 'PawnFilesScreenViewport()', 1),
    ('GetLevel()->Engine->Client->MaintainVerticalFOV', 'PawnFilesMaintainVertical()', 1),
    ('UClient::GetEffectiveFovAngle(', 'PawnFilesEffectiveFov(', 1),
    ('eventPlayerCalcView( ViewActor, ViewLocation, ViewRotation );',
     'PawnFilesCalcView( ViewActor, ViewLocation, ViewRotation );', 1),
    ('Player->Exec( *Command, StrOut );', 'PawnFilesPlayerExec( Player, *Command, StrOut );', 1),
    ('GetLevel()->Engine->Exec( *Command, StrOut );', 'PawnFilesEngineExec( GetLevel()->Engine, *Command, StrOut );', 1),
]

spans = list(span_re.finditer(s))
assert len(spans) == len(names), len(spans)
counts = {old: 0 for old, _, _ in rules}
level_re = re.compile(r'(?<![\w>.])GetLevel\(\)')
level_count = 0
out, last = [], 0
for m in spans:
    out.append(s[last:m.start()])
    body = m.group(0)
    for old, new, _ in rules:
        counts[old] += body.count(old)
        body = body.replace(old, new)
    # GetLevel() last, so the earlier rewrites see the original engine expressions.
    body, n = level_re.subn('PawnFilesGetLevel(this)', body)
    level_count += n
    out.append(body)
    last = m.end()
out.append(s[last:])
for old, _, want in rules:
    assert counts[old] == want, (old, counts[old], want)
assert level_count > 0, level_count
p.write_bytes(''.join(out).encode('utf-8'))
PY
python3 - "$CC_DB" "$REPO" "$WORK" > "$WORK/flags.txt" <<'PY'
import json, shlex, sys
db, repo, work = sys.argv[1:4]
e = [x for x in json.load(open(db)) if x['file'].endswith('UnPawn.cpp')][0]
toks = shlex.split(e['command'].replace(repo, work))
out, skip = [], False
for t in toks:
    if skip: skip = False; continue
    if t in ('-o', '-MF', '-MT'): skip = True; continue
    if t in ('-MD', '-MMD', '-c') or t.startswith('-O') or t.endswith('.cpp') or t.endswith('c++'): continue
    out.append(t)
print(' '.join(shlex.quote(t) for t in out))
PY
FLAGS=$(cat "$WORK/flags.txt")
cd "$SRC"
eval "c++ $FLAGS -O3 -g -c UnPawn.cpp -o $WORK/UnPawn.o"
eval "c++ $FLAGS -O3 -g -c $WORK/HarryPotter2/Unreal/Core/Src/UnMath.cpp -o $WORK/UnMath.o"
eval "c++ $FLAGS -O3 -g -c $WORK/HarryPotter2/Unreal/Core/Src/UnAnsi.cpp -o $WORK/UnAnsi.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/pawn_native_files_fixture_harness/pawn_native_files_harness.cpp -o $WORK/harness.o"
{ nm -g --defined-only "$WORK/harness.o"; nm -g --defined-only "$WORK/UnMath.o"; nm -g --defined-only "$WORK/UnAnsi.o"; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ nm -u "$WORK/UnPawn.o"; nm -u "$WORK/UnMath.o"; nm -u "$WORK/UnAnsi.o"; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
python3 - "$WORK/syms.txt" > "$WORK/stubs.cpp" <<'PY'
import sys
syms=[line.strip() for line in open(sys.argv[1]) if line.strip()]
forbidden=('FindFunctionChecked','ProcessEvent','IsA','IsProbing','IsValid','eventHearNoise',
           'eventEnemyNotVisible','eventTouch','eventSpecialHandling','appFrand',
           'FastLineCheck','SingleLineCheck','GetCylinderExtent','setPhysics',
           'SafeNormal','Rotation','GMath','appSin','appPow','appIsNan',
           'appAtan2','appRound','appCycles','GetStateFrame','GetSuperState','GetFName',
           'StaticClass','GetPrivateStaticClass','GetPathName','GetLevelInfo')
out=[]
for i,s in enumerate(syms):
    # Engine C++ the harness does not provide: any such function reached aborts.
    # Vtables and typeinfo get typed, zeroed pointer storage: the fixture never makes a
    # virtual call on these objects. Other non-C++ symbols get no stub: libc resolves
    # dynamically, and a project global the harness lacks fails the link.
    # libc++ and operator new/delete resolve from the system libraries; they are not engine code.
    is_fn = s.startswith('_Z') and not s.startswith(('_ZT', '_ZS', '_ZNSt', '_ZNKSt', '_Zn', '_Zd'))
    if s == '_ZN9__Context3EnvE':
        # Data, not code: the guard() jmp_buf that setjmp writes into.
        out.append(f'jmp_buf ctx_env __asm__("_{s}");\n')
    elif is_fn or any(name in s for name in forbidden):
        out.append(f'extern "C" void trap{i}(void) __asm__("_{s}");\nvoid trap{i}(void) {{ fputs("trap {s}\\n", stderr); abort(); }}\n')
    elif s.startswith(('_ZTV', '_ZTI')):
        out.append(f'__attribute__((used)) void* storage{i}[256] __asm__("_{s}");\n')
print('#include <cstdio>\n#include <cstdlib>\n#include <setjmp.h>\n' + ''.join(out))
PY
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnPawn.o" "$WORK/UnMath.o" "$WORK/UnAnsi.o" "$WORK/stubs.o" -o "$WORK/pawn_native_files_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/pawn_native_files.txt
TMP1=$OUT.tmp.1
TMP2=$OUT.tmp.2
"$WORK/pawn_native_files_harness" > "$TMP1"
"$WORK/pawn_native_files_harness" > "$TMP2"
cmp "$TMP1" "$TMP2"
test "$(grep -c '^entry=get_next_int ' "$TMP1")" -eq 4
test "$(grep -c '^entry=get_next_int_desc ' "$TMP1")" -eq 3
test "$(grep -c '^entry=get_cache_entry ' "$TMP1")" -eq 4
test "$(grep -c '^entry=move_cache_entry ' "$TMP1")" -eq 8
test "$(grep -c '^entry=check_valid_skin_package ' "$TMP1")" -eq 11
test "$(grep -c '^entry=update_url ' "$TMP1")" -eq 2
test "$(grep -c '^entry=get_default_url ' "$TMP1")" -eq 3
test "$(grep -c '^entry=get_next_skin ' "$TMP1")" -eq 7
test "$(grep -c '^entry=reset_keyboard ' "$TMP1")" -eq 3
test "$(grep -c '^entry=find_best_inventory_path ' "$TMP1")" -eq 6
test "$(grep -c '^entry=get_map_name ' "$TMP1")" -eq 10
test "$(grep -c '^entry=console_command ' "$TMP1")" -eq 3
test "$(grep -c '^entry=screen_to_world ' "$TMP1")" -eq 4
test "$(grep -c '^entry=get_url_map ' "$TMP1")" -eq 3
test "$(grep -c '^entry=get_entry_level ' "$TMP1")" -eq 1
test "$(wc -l < "$TMP1")" -eq 72
mv "$TMP1" "$OUT"
rm "$TMP2"
printf 'wrote %s records\n' "$(wc -l < "$OUT")"
