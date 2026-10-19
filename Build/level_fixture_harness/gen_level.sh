#!/bin/sh
# Regenerate the UnLevel oracle fixture from the original UnLevel.cpp at 7ddb1d3.
# Usage: gen_level.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=7ddb1d3
WORK=$(mktemp -d)
cleanup() { [ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
c++ -std=c++17 -I"$REPO/HarryPotter2/Unreal/Engine/Inc" "$REPO/Build/level_fixture_harness/ffi_exception_test.cpp" -o "$WORK/ffi_exception_test"
"$WORK/ffi_exception_test"
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
SRC="$WORK/HarryPotter2/Unreal/Engine/Src"
FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/UnLevel.cpp "$REPO" "$WORK")
UFLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/UnEngine.cpp "$REPO" "$WORK")
MISCFLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Core/Src/UnMisc.cpp "$REPO" "$WORK")
ANSIFLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Core/Src/UnAnsi.cpp "$REPO" "$WORK")
UNMISC=$(find "$WORK/HarryPotter2" -name UnMisc.cpp | head -1)
UNANSI=$(find "$WORK/HarryPotter2" -name UnAnsi.cpp | head -1)
cd "$SRC"
python3 - "$SRC/UnLevel.cpp" <<'PY'
import sys
p = sys.argv[1]
s = open(p, 'rb').read().decode('latin-1')
def rep(s, a, b, n):
    assert s.count(a) == n, (a, s.count(a))
    return s.replace(a, b)
# Zeroed harness ULevel has no vtable: route virtual ULevel calls to harness hooks.
s = rep(s, 'DestroyActor( pAct );', 'harness_destroy_actor( this, pAct );', 1)
s = rep(s, 'else DestroyActor( View );', 'else harness_destroy_actor( this, View );', 1)
s = rep(s, 'SpawnActor( ALevelInfo::StaticClass() );', 'harness_spawn_actor( this, ALevelInfo::StaticClass() );', 1)
s = rep(s, 'ABrush* Temp = SpawnBrush();', 'ABrush* Temp = harness_spawn_brush( this );', 1)
s = rep(s, 'return SingleLineCheck( Hit, SoundActor, SoundLocation, ListenerLocation, TRACE_VisBlocking );', 'return harness_single_line_check( this, Hit, SoundActor, SoundLocation, ListenerLocation, TRACE_VisBlocking );', 1)
s = rep(s, 'if( SingleLineCheck( FirstHit, SourceActor, End, StartTrace, TraceFlags, Extent ) )', 'if( harness_single_line_check( this, FirstHit, SourceActor, End, StartTrace, TraceFlags, Extent ) )', 2)
s = rep(s, 'pAct->eventOnResolveGameState();', 'harness_on_resolve_game_state( pAct );', 1)
s = rep(s, 'GetLevelInfo()->Game->eventDetailChange();', 'harness_event_detail_change( GetLevelInfo()->Game );', 1)
s = rep(s, 'SpawnViewActor( Viewport );', 'harness_spawn_view_actor( this, Viewport );', 1)
s = rep(s, 'Player->IsZoneVisible( Node->iZone[1] )', 'harness_is_zone_visible( Player, Node->iZone[1] )', 1)
s = rep(s, 'Player->IsZoneVisible( Check->Actor->Region.ZoneNumber )', 'harness_is_zone_visible( Player, Check->Actor->Region.ZoneNumber )', 2)
s = rep(s, 'NetDriver->Exec( Cmd, Ar )', 'harness_net_exec( NetDriver, Cmd, Ar )', 1)
s = rep(s, 'DemoRecDriver->Exec( Cmd, Ar )', 'harness_net_exec( DemoRecDriver, Cmd, Ar )', 1)
s = rep(s, 'delete DemoRecDriver;', 'harness_delete_driver( DemoRecDriver );', 2)
s = rep(s, 'delete NetDriver;', 'harness_delete_driver( NetDriver );', 2)
s = rep(s, 'DemoRecDriver->InitListen( this, URL, Error )', 'harness_init_listen( DemoRecDriver, this, URL, Error )', 1)
s = rep(s, 'LocalizeError("NetAlready")', 'harness_localize_error("NetAlready")', 1)
s = rep(s, 'LocalizeError("NetListen")', 'harness_localize_error("NetListen")', 1)
s = rep(s, 'NetDriver->InitListen( this, URL, Error )', 'harness_init_listen( NetDriver, this, URL, Error )', 1)
s = rep(s, 'GameEngine->BuildServerMasterMap( NetDriver, this )', 'harness_build_master_map( GameEngine, NetDriver, this )', 1)
s = rep(s, 'SpawnActor( HelperClass )', 'harness_spawn_helper( this, HelperClass )', 1)
s = rep(s, 'It->ImportText( Value, (BYTE*)Actor + It->Offset, 0 )', 'harness_import_text( *It, Value, (BYTE*)Actor + It->Offset )', 1)
s = rep(s, 'TFieldIterator<UProperty> It(Actor->GetClass())', 'HarnessPropIterator It(Actor->GetClass())', 1)
s = rep(s, 'Connection->PackageMap->Copy( Connection->Driver->MasterMap )', 'harness_package_map_copy( Connection )', 1)
s = rep(s, 'Connection->Logf(', 'harness_conn_logf( Connection, ', 8)
s = rep(s, 'Connection->Channels[0]->Close();', 'harness_channel_close( Connection->Channels[0] );', 4)
s = rep(s, 'Connection->SendPackageMap()', 'harness_send_package_map( Connection )', 1)
s = rep(s, 'Connection->FlushNet()', 'harness_flush_net( Connection )', 6)
s = rep(s, 'Connection->LowLevelGetRemoteAddress()', 'harness_remote_address( Connection )', 2)
s = rep(s, 'appTimestamp()', 'harness_timestamp()', 1)
s = rep(s, 'appCycles()', 'harness_cycles()', 1)
s = rep(s, 'Engine->SetClientTravel( Engine->Client->Viewports(0), TEXT("?failed"), 0, TRAVEL_Absolute );', 'harness_set_client_travel( Engine, Engine->Client->Viewports(0), TEXT("?failed"), 0, TRAVEL_Absolute );', 1)
s = rep(s, 'Engine->ChallengeResponse(Connection->Challenge)', 'harness_challenge_response( Engine, Connection->Challenge )', 1)
s = rep(s, 'GetLevelInfo()->Game->eventPreLogin( Tmp, harness_remote_address( Connection ), Error, FailCode )', 'harness_pre_login( GetLevelInfo()->Game, Tmp, harness_remote_address( Connection ), Error, FailCode )', 1)
s = rep(s, 'WelcomePlayer( Connection );', 'ULevel::WelcomePlayer( Connection, TEXT("") );', 1)
k = s.index('\n', s.index('#include <stdlib.h>')) + 1
k_line = s[:k].count('\n') + 1
s = s[:k] + '#line %d "UnLevel.cpp"\n' % k_line + s[k:]
s = s[:k] + 'UBOOL harness_destroy_actor( ULevel* Level, AActor* Actor );\nAActor* harness_spawn_actor( ULevel* Level, UClass* Class );\nABrush* harness_spawn_brush( ULevel* Level );\nUBOOL harness_single_line_check( ULevel* Level, FCheckResult& Hit, AActor* SourceActor, const FVector& End, const FVector& Start, DWORD TraceFlags, FVector Extent = FVector(0,0,0) );\n' + s[k:]
s = s[:k] + 'void harness_on_resolve_game_state( AActor* Actor );\n' + s[k:]
s = s[:k] + 'void harness_event_detail_change( AGameInfo* Game );\n' + s[k:]
s = s[:k] + 'void harness_spawn_view_actor( ULevel* Level, UViewport* Viewport );\n' + s[k:]
s = s[:k] + 'class APlayerPawn;\nbool harness_is_zone_visible( APlayerPawn* Player, INT iZone );\n' + s[k:]
s = s[:k] + 'UBOOL harness_net_exec( UNetDriver* Driver, const TCHAR* Cmd, FOutputDevice& Ar );\nvoid harness_delete_driver( UNetDriver* Driver );\nUBOOL harness_init_listen( UNetDriver* Driver, ULevel* Level, FURL& URL, FString& Error );\n' + s[k:]
s = s[:k] + 'void harness_package_map_copy( UNetConnection* Connection );\nvoid harness_send_package_map( UNetConnection* Connection );\nvoid harness_flush_net( UNetConnection* Connection );\nvoid harness_conn_logf( UNetConnection* Connection, const TCHAR* Fmt, ... );\nvoid harness_channel_close( UChannel* Channel );\n' + s[k:]
s = s[:k] + 'FString harness_remote_address( UNetConnection* Connection );\nconst TCHAR* harness_timestamp();\nDWORD harness_cycles();\nconst TCHAR* harness_localize_error( const ANSICHAR* Key );\nvoid harness_build_master_map( UGameEngine* Game, UNetDriver* Driver, ULevel* Level );\nAActor* harness_spawn_helper( ULevel* Level, UClass* Class );\nvoid harness_import_text( UProperty* Property, const TCHAR* Value, BYTE* Data );\nvoid harness_set_client_travel( UEngine* Engine, UPlayer* Viewport, const TCHAR* URL, UBOOL Items, ETravelType Type );\nINT harness_challenge_response( UEngine* Engine, INT Challenge );\nvoid harness_pre_login( AGameInfo* Game, const FString& Options, const FString& Address, FString& Error, FString& FailCode );\n' + s[k:]
s = s[:k] + 'struct HarnessPropIterator\n{\n\tUStruct* Struct;\n\tUField* Field;\n\tHarnessPropIterator( UStruct* InStruct ) : Struct( InStruct ), Field( InStruct ? InStruct->Children : NULL ) { Advance(); }\n\toperator UBOOL() { return Field != NULL; }\n\tvoid operator++() { Field = Field->Next; Advance(); }\n\tUProperty* operator*() { return (UProperty*)Field; }\n\tUProperty* operator->() { return (UProperty*)Field; }\n\tvoid Advance()\n\t{\n\t\twhile( Struct )\n\t\t{\n\t\t\twhile( Field )\n\t\t\t{\n\t\t\t\tif( Field->IsA(UProperty::StaticClass()) ) return;\n\t\t\t\tField = Field->Next;\n\t\t\t}\n\t\t\tStruct = (UStruct*)Struct->SuperField;\n\t\t\tif( Struct ) Field = Struct->Children;\n\t\t}\n\t}\n};\n' + s[k:]
open(p, 'wb').write(s.encode('latin-1'))
PY
c++ $FLAGS -O3 -g -c UnLevel.cpp -o "$WORK/UnLevel.o"
c++ $MISCFLAGS -O3 -g -c "$UNMISC" -o "$WORK/UnMisc.o"
c++ $ANSIFLAGS -O3 -g -c "$UNANSI" -o "$WORK/UnAnsi.o"
UNOBJ=$(find "$WORK/HarryPotter2" -name UnObj.cpp | head -1)
python3 - "$REPO/Build/level_fixture_harness/level_harness.cpp" "$UNOBJ" "$WORK/harness.cpp" <<'PY'
import sys
h, unobj, out = sys.argv[1:4]
src = open(unobj, 'rb').read().decode('latin-1').replace('\r\n', '\n')
start = src.index('FArchive& operator<<( FArchive& Ar, FCompactIndex& I )\n{')
fn = src[start:src.index('\n}\n', start) + 3]
body = open(h, 'rb').read().decode('latin-1')
body += '\n// FCompactIndex serializer: verbatim from Core/Src/UnObj.cpp at the oracle ref (compiled unmodified).\n' + fn
open(out, 'wb').write(body.encode('latin-1'))
PY
c++ $FLAGS -O3 -g -fno-access-control -c "$WORK/harness.cpp" -o "$WORK/harness.o"
OBJS="$WORK/UnLevel.o $WORK/UnMisc.o $WORK/UnAnsi.o $WORK/harness.o"
nm -g --defined-only $OBJS | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ for o in $OBJS; do nm -u "$o"; done; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|__chkstk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi|__dso|__tlv|dyld|pow$|ceil$|floor$|sqrt$|sin$|cos$|tan$|atan$|exp$|log$|fabs$|mem[a-z]*$|str[a-z]*$|printf$|puts$|free$|malloc$|realloc$|calloc$|abort$|exit$|qsort$|snprintf$|vsnprintf$|getenv$|time$|localtime$|gettimeofday$|clock$|sprintf$|fopen$|fclose$|fread$|fwrite$|fputs$|fprintf$|fflush$|putchar$|wcslen$|wcscmp$|wcsncmp$|towlower$|towupper$|iswspace$|_Znw|_Zdl|_Znam|_Zda)' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" $OBJS > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnLevel.o" "$WORK/UnMisc.o" "$WORK/UnAnsi.o" "$WORK/stubs.o" -o "$WORK/level_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/level_oracle.txt
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/level_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/level_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
for fam in persist core net; do
  test "$(grep -c "^scenario[[:space:]]$fam\." "$WORK/run1.txt")" -gt 0
done
cp "$WORK/run1.txt" "$OUT"
printf 'wrote %s lines\n' "$(wc -l < "$OUT")"
