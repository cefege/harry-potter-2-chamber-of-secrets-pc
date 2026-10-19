#!/bin/sh
# Regenerate the UnEngine oracle fixture from original UnEngine.cpp at 6bd3da5.
# Usage: gen_uengine.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=6bd3da5
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$WORK/HarryPotter2/Unreal/Engine/Src/UnEngine.cpp" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
s = p.read_bytes().decode('utf-8').replace('\r\n', '\n')
anchor = '#include "EnginePrivate.h"'
decls = '''
// Oracle hooks (Build/uengine_fixture_harness); engine calls routed to the harness.
UBOOL EngParseParam(const TCHAR* Param);
void* EngLoadAudioClass();
void* EngConstructAudio(void* Class);
UBOOL EngAudioInit(void* Audio);
void EngLogAudioInitFailed();
void EngAudioDelete(void* Audio);
void EngEngineMemInit(INT Bytes, const TCHAR* Name);
void EngEngineMemExit();
void EngCacheInit(INT Bytes, INT Align);
void EngCacheExit(INT FreeMemory);
void EngCacheFlush();
UBOOL EngCacheExec(const TCHAR* Cmd, FOutputDevice& Ar);
void EngUrlStaticInit();
void EngUrlStaticExit();
const TCHAR* EngLocalize(const char* Section, const char* Key, const TCHAR* Package);
void* EngNewObject(const char* Kind);
void EngAddToRoot();
void EngRemoveFromRoot();
void EngLogEngineInitialized();
UBOOL EngSysExec(void* Sys, const TCHAR* Cmd, FOutputDevice& Ar);
UBOOL EngStaticExec(const TCHAR* Cmd, FOutputDevice& Ar);
UBOOL EngExecExec(void* Exec, const TCHAR* Cmd, FOutputDevice& Ar);
UBOOL EngClientExec(void* Client, const TCHAR* Cmd, FOutputDevice& Ar);
UBOOL EngRenderExec(void* Render, const TCHAR* Cmd, FOutputDevice& Ar);
UBOOL EngAudioExec(void* Audio, const TCHAR* Cmd, FOutputDevice& Ar);
void EngClientFlush(void* Client, INT AllowPrecache);
void EngOutLog(FOutputDevice& Ar, const TCHAR* Text);
void EngOutLogf(FOutputDevice& Ar, const TCHAR* Fmt);
void EngOutLogfS(FOutputDevice& Ar, const TCHAR* Fmt, const TCHAR* S);
void EngOutLogfI(FOutputDevice& Ar, const TCHAR* Fmt, INT I);
void EngOutLogfIS(FOutputDevice& Ar, const TCHAR* Fmt, INT I, const TCHAR* S);
void* EngUrlCrack(const TCHAR* Cmd);
UBOOL EngUrlValid(void* URL);
const TCHAR* EngUrlProtocol(void* URL);
const TCHAR* EngUrlHost(void* URL);
INT EngUrlPort(void* URL);
const TCHAR* EngUrlMap(void* URL);
INT EngUrlNumOptions(void* URL);
const TCHAR* EngUrlOption(void* URL, INT I);
const TCHAR* EngUrlPortal(void* URL);
const TCHAR* EngUrlString(void* URL);
void EngUrlFree(void* URL);
UBOOL EngConsoleEventKeyType(void* Console, INT Key);
UBOOL EngConsoleEventKeyEvent(void* Console, INT Key, INT State, FLOAT Delta);
UBOOL EngInputPreProcess(void* Input, INT Key, INT State, FLOAT Delta);
UBOOL EngInputProcess(void* Input, FOutputDevice& Ar, INT Key, INT State, FLOAT Delta);
void* EngObjectClass(void* Obj);
void* EngClassSuper(void* Class);
const TCHAR* EngClassName(void* Class);
UBOOL EngConfigGetLanguage(TCHAR* Buf, INT Len);
void EngSetLanguage(const TCHAR* Text);
void* EngLoadEngineClass();
void* EngConstructEngine(void* Class);
void EngEngineTick(void* Engine, FLOAT Delta);
void EngSetCurrentTickRate(void* Engine, FLOAT Rate);
void EngAppSleep(FLOAT Seconds);
FTime EngAppSeconds();
'''
def rep(old, new, count):
    global s
    n = s.count(old)
    assert n == count, (old, n)
    s = s.replace(old, new)

def rrep(pat, new, count):
    global s
    s, n = re.subn(pat, new, s, flags=re.S)
    assert n == count, (pat, n)

# Top-level NAMES_ONLY expansion registers script natives (GRegisterNative at static init); shell-only.
rrep(r'// Register things\.\n#define NAMES_ONLY\n.*?#undef NAMES_ONLY\n', '', 1)
# Init: intrinsic-name block is shell-only.
rrep(r'\t// Add the intrinsic names\.\n.*?\t#undef NAMES_ONLY\n', '', 1)

# InitAudio.
rep('ParseParam(appCmdLine(),TEXT("NOSOUND"))', 'EngParseParam(TEXT("NOSOUND"))', 1)
rep('UClass* AudioClass = StaticLoadClass( UAudioSubsystem::StaticClass(), NULL, TEXT("ini:Engine.Engine.AudioDevice"), NULL, LOAD_NoFail, NULL );',
    'UClass* AudioClass = (UClass*)EngLoadAudioClass();', 1)
rep('ConstructObject<UAudioSubsystem>( AudioClass )', '(UAudioSubsystem*)EngConstructAudio( AudioClass )', 1)
rep('Audio->Init()', 'EngAudioInit( Audio )', 1)
rep('debugf( NAME_Log, TEXT("Audio initialization failed.") );', 'EngLogAudioInitFailed();', 1)
rep('delete Audio;', 'EngAudioDelete( Audio );', 2)

# Init.
rep('FURL::StaticInit();', 'EngUrlStaticInit();', 1)
rep('GEngineMem.Init( 65536, TEXT("GEngineMem") );', 'EngEngineMemInit( 65536, TEXT("GEngineMem") );', 1)
rep('GCache.Init(', 'EngCacheInit(', 1)
rep('*Localize( "General", "Yes", TEXT("Core") )', '*EngLocalize( "General", "Yes", TEXT("Core") )', 1)
rep('*Localize( "General", "No",  TEXT("Core") )', '*EngLocalize( "General", "No",  TEXT("Core") )', 1)
rrep(r'new (UCylinder|UOrientedCylinder|UAlignedOvalCylinder|UOrientedOvalCylinder|UBox);',
     lambda m: '(%s*)EngNewObject("%s");' % (m.group(1), m.group(1)), 5)
rep('AddToRoot();', 'EngAddToRoot();', 1)
rep('debugf( NAME_Init, TEXT("Unreal engine initialized") );', 'EngLogEngineInitialized();', 1)
rep('Engine->Init();', 'Engine->UEngine::Init();', 1)

# Exit / Destroy.
rep('RemoveFromRoot();', 'EngRemoveFromRoot();', 1)
rep('FURL::StaticExit();', 'EngUrlStaticExit();', 1)
rep('GEngineMem.Exit();', 'EngEngineMemExit();', 1)
rep('GCache.Exit( 1 );', 'EngCacheExit( 1 );', 1)
rep('\tSuper::Destroy();\n', '', 1)

# Flush.
rep('GCache.Flush();', 'EngCacheFlush();', 1)
rep('Client->Flush( AllowPrecache );', 'EngClientFlush( Client, AllowPrecache );', 1)

# Exec: claims, then FLUSH and CRACKURL.
rep('GSys->Exec\t\t(Cmd,Ar)', 'EngSysExec(GSys,Cmd,Ar)', 1)
rep('UObject::StaticExec\t\t\t(Cmd,Ar)', 'EngStaticExec(Cmd,Ar)', 1)
rep('GCache.Exec\t\t\t\t\t(Cmd,Ar)', 'EngCacheExec(Cmd,Ar)', 1)
rep('GExec->Exec      (Cmd,Ar)', 'EngExecExec(GExec,Cmd,Ar)', 1)
rep('Client->Exec\t\t(Cmd,Ar)', 'EngClientExec(Client,Cmd,Ar)', 1)
rep('Render->Exec\t\t(Cmd,Ar)', 'EngRenderExec(Render,Cmd,Ar)', 1)
rep('Audio->Exec\t\t(Cmd,Ar)', 'EngAudioExec(Audio,Cmd,Ar)', 1)
rep('Flush(1);', 'UEngine::Flush(1);', 1)
rep('Ar.Log( TEXT("Flushed engine caches") );', 'EngOutLog( Ar, TEXT("Flushed engine caches") );', 1)
rep('FURL URL(NULL,Cmd,TRAVEL_Absolute);', 'void* URL = EngUrlCrack(Cmd);', 1)
rep('if( URL.Valid )', 'if( EngUrlValid(URL) )', 1)
rep('Ar.Logf( TEXT("     Protocol: %s"), *URL.Protocol );', 'EngOutLogfS( Ar, TEXT("     Protocol: %s"), EngUrlProtocol(URL) );', 1)
rep('Ar.Logf( TEXT("         Host: %s"), *URL.Host );', 'EngOutLogfS( Ar, TEXT("         Host: %s"), EngUrlHost(URL) );', 1)
rep('Ar.Logf( TEXT("         Port: %i"), URL.Port );', 'EngOutLogfI( Ar, TEXT("         Port: %i"), EngUrlPort(URL) );', 1)
rep('Ar.Logf( TEXT("          Map: %s"), *URL.Map );', 'EngOutLogfS( Ar, TEXT("          Map: %s"), EngUrlMap(URL) );', 1)
rep('Ar.Logf( TEXT("   NumOptions: %i"), URL.Op.Num() );', 'EngOutLogfI( Ar, TEXT("   NumOptions: %i"), EngUrlNumOptions(URL) );', 1)
rep('i<URL.Op.Num();', 'i<EngUrlNumOptions(URL);', 1)
rep('Ar.Logf( TEXT("     Option %i: %s"), i, *URL.Op(i) );', 'EngOutLogfIS( Ar, TEXT("     Option %i: %s"), i, EngUrlOption(URL, i) );', 1)
rep('Ar.Logf( TEXT("       Portal: %s"), *URL.Portal );', 'EngOutLogfS( Ar, TEXT("       Portal: %s"), EngUrlPortal(URL) );', 1)
rep('Ar.Logf( TEXT("       String: \'%s\'"), **URL.String() );', 'EngOutLogfS( Ar, TEXT("       String: \'%s\'"), EngUrlString(URL) );', 1)
rep('\t\telse Ar.Logf( TEXT("BAD URL") );\n\t\treturn 1;',
    '\t\telse EngOutLogf( Ar, TEXT("BAD URL") );\n\t\tEngUrlFree(URL);\n\t\treturn 1;', 1)

# Key and InputEvent.
rep('Viewport->Console->eventKeyType( Key )', 'EngConsoleEventKeyType( Viewport->Console, Key )', 1)
rep('C=C->GetSuperClass()', 'C=(UClass*)EngClassSuper( C )', 1)
rep('Viewport->Actor->myHUD->MainMenu->GetClass()', '(UClass*)EngObjectClass( Viewport->Actor->myHUD->MainMenu )', 1)
rep('C->GetName()', 'EngClassName( C )', 2)
rep('Viewport->Console->eventKeyEvent( iKey, State, Delta )', 'EngConsoleEventKeyEvent( Viewport->Console, iKey, State, Delta )', 1)
rep('Viewport->Input->PreProcess( iKey, State, Delta )', 'EngInputPreProcess( Viewport->Input, iKey, State, Delta )', 2)
rep('Viewport->Input->Process( ', 'EngInputProcess( Viewport->Input, ', 1)

# Commandlet Main.
rep('GConfig->GetString( TEXT("Engine.Engine"), TEXT("Language"), Temp, ARRAY_COUNT(Temp) )', 'EngConfigGetLanguage( Temp, ARRAY_COUNT(Temp) )', 1)
rep('UObject::SetLanguage( Temp );', 'EngSetLanguage( Temp );', 1)
rep('UObject::StaticLoadClass( UEngine::StaticClass(), NULL, TEXT("ini:Engine.Engine.GameEngine"), NULL, LOAD_NoFail | LOAD_DisallowFiles, NULL )', '(UClass*)EngLoadEngineClass()', 1)
rep('ConstructObject<UEngine>( EngineClass )', '(UEngine*)EngConstructEngine( EngineClass )', 1)
rep('appSeconds()', 'EngAppSeconds()', 3)
rep('Engine->Tick( NewTime - OldTime );', 'EngEngineTick( Engine, NewTime - OldTime );', 1)
rep('Engine->CurrentTickRate = (FLOAT)TickCount / (OldTime - SecondStartTime);', 'EngSetCurrentTickRate( Engine, (FLOAT)TickCount / (OldTime - SecondStartTime) );', 1)
rep('Engine->GetMaxTickRate()', 'Engine->UEngine::GetMaxTickRate()', 1)
rep('appSleep( Max(0.f,Delta) );', 'EngAppSleep( Max(0.f,Delta) );', 1)

# Every routed external call must be gone from the oracle body.
for marker in ['GCache.', 'GEngineMem.', 'GConfig->', 'Ar.Log', 'appCmdLine', 'StaticLoadClass',
               'ConstructObject<', 'FURL::', 'FURL URL', 'appSeconds()', 'Super::Destroy',
               'GSys->Exec', 'GExec->Exec', 'Client->Exec', 'Render->Exec', 'Audio->Exec',
               'Audio->Init', 'Engine->Tick', 'Engine->Init', '->GetClass()', '->GetName()',
               '->GetSuperClass()', 'eventKeyType', 'eventKeyEvent', '->PreProcess(', '->Process(',
               'Localize( "General"']:
    assert marker not in s.replace('EngLocalize( "General"', ''), marker
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + '\n' + decls, 1)
p.write_bytes(s.encode('utf-8'))
PY
FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/UnEngine.cpp "$REPO" "$WORK")
FLAGS_MISC=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Core/Src/UnMisc.cpp "$REPO" "$WORK")
FLAGS_ANSI=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Core/Src/UnAnsi.cpp "$REPO" "$WORK")
UNMISC=$(find "$WORK/HarryPotter2" -name UnMisc.cpp | head -1)
UNANSI=$(find "$WORK/HarryPotter2" -name UnAnsi.cpp | head -1)
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnEngine.cpp -o $WORK/UnEngine.o"
eval "c++ $FLAGS_MISC -O3 -g -c $UNMISC -o $WORK/UnMisc.o"
eval "c++ $FLAGS_ANSI -O3 -g -c $UNANSI -o $WORK/UnAnsi.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/uengine_fixture_harness/uengine_harness.cpp -o $WORK/harness.o"
OBJS="$WORK/UnEngine.o $WORK/UnMisc.o $WORK/UnAnsi.o $WORK/harness.o"
{ for o in $OBJS; do nm -g --defined-only "$o"; done; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ for o in $OBJS; do nm -u "$o"; done; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|__chkstk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|memset_pattern[0-9]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+|wcsn[a-z]+|atof|putchar)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled char[64]. Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" $OBJS > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnEngine.o" "$WORK/UnMisc.o" "$WORK/UnAnsi.o" "$WORK/stubs.o" -o "$WORK/uengine_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/uengine_oracle.txt
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/uengine_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/uengine_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
test "$(grep -c '^scenario' "$WORK/run1.txt")" -eq 44
cp "$WORK/run1.txt" "$OUT"
printf 'wrote %s lines\n' "$(wc -l < "$OUT")"
