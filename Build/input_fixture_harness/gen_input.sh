#!/bin/sh
# Regenerate the input-system oracle fixture from original UnIn.cpp at 4914c7e.
# Usage: gen_input.sh <build-dir-with-compile_commands.json>
set -eu
export LC_ALL=C
REPO=$(cd "$(dirname "$0")/../.." && pwd)
CC_DB=$1/compile_commands.json
REF=4914c7e
WORK=$(mktemp -d)
trap '[ -n "${KEEP_WORK:-}" ] || rm -rf "$WORK"' EXIT
git -C "$REPO" archive "$REF" | tar -x -C "$WORK"
python3 - "$WORK/HarryPotter2/Unreal/Engine/Src/UnIn.cpp" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_bytes().decode('utf-8').replace('\r\n', '\n')
anchor = '#include "EnginePrivate.h"'
decls = '''
// Oracle hooks (Build/input_fixture_harness); engine calls routed to the harness.
template<class T> struct InputKindOf;
template<> struct InputKindOf<UProperty> { enum { K = 0 }; };
template<> struct InputKindOf<UByteProperty> { enum { K = 1 }; };
template<> struct InputKindOf<UFloatProperty> { enum { K = 2 }; };
UProperty* InputNthField(UClass* Class, INT Kind, INT N);
template<class T> struct InputFieldIter {
	UClass* Owner;
	INT N;
	T* Cur;
	InputFieldIter( UClass* InOwner ) : Owner(InOwner), N(0), Cur((T*)InputNthField(InOwner, InputKindOf<T>::K, 0)) {}
	operator bool() const { return Cur != NULL; }
	InputFieldIter& operator++() { Cur = (T*)InputNthField(Owner, InputKindOf<T>::K, ++N); return *this; }
	T* operator*() const { return Cur; }
	T* operator->() const { return Cur; }
};
void* InputCacheGet(QWORD CacheId, FMemCache::FCacheItem*& Item);
void* InputCacheCreate(QWORD CacheId, FMemCache::FCacheItem*& Item, SIZE_T Size);
void InputCacheUnlock(FMemCache::FCacheItem* Item);
EName InputFindName(const TCHAR* Text);
const TCHAR* InputKeyChars(INT Key);
UBOOL InputKeyFindItem(FName N, INT& Index);
UByteProperty* InputCastByte(UProperty* P);
UFloatProperty* InputCastFloat(UProperty* P);
APlayerPawn* InputCastPawn(AActor* A);
UViewport* InputCastViewport(UPlayer* P);
void InputViewportExec(UViewport* Viewport, const TCHAR* Str, FOutputDevice& Ar);
void InputViewportUpdate(UViewport* Viewport, INT Flag);
void InputLogInit(UViewport* Viewport);
BYTE InputArgByte();
'''
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + '\n' + decls, 1)
repls = [
    ('GCache.Get(CacheId,Item)', 'InputCacheGet(CacheId,Item)', 1),
    ('GCache.Create(', 'InputCacheCreate(', 1),
    ('Item->Unlock();', 'InputCacheUnlock(Item);', 3),
    ('TFieldIterator<UProperty>', 'InputFieldIter<UProperty>', 2),
    ('TFieldIterator<UByteProperty>', 'InputFieldIter<UByteProperty>', 1),
    ('TFieldIterator<UFloatProperty>', 'InputFieldIter<UFloatProperty>', 1),
    ('Cast<UByteProperty>(', 'InputCastByte(', 1),
    ('Cast<UFloatProperty>(', 'InputCastFloat(', 2),
    ('Cast<APlayerPawn>(this)', 'InputCastPawn(this)', 1),
    ('Cast<UViewport>(PlayerPawn->Player)', 'InputCastViewport(PlayerPawn->Player)', 1),
    ('FName Button( ButtonName, FNAME_Find );', 'FName Button( InputFindName(ButtonName) );', 2),
    ('FName Name(Temp,FNAME_Find);', 'FName Name( InputFindName(Temp) );', 1),
    ('FName N( Temp, FNAME_Find );', 'FName N( InputFindName(Temp) );', 1),
    ('*InputKeys->Names(Key)', 'InputKeyChars(Key)', 2),
    ('InputKeys->Names.FindItem( N, *(INT*)&iKey )', 'InputKeyFindItem( N, *(INT*)&iKey )', 1),
    ('Viewport->Exec( Str, Ar );', 'InputViewportExec( Viewport, Str, Ar );', 1),
    ('Viewport->UpdateInput( 1 );', 'InputViewportUpdate( Viewport, 1 );', 1),
    ('debugf( NAME_Init, TEXT("Input system initialized for %s"), Viewport->GetName() );',
     'InputLogInit( Viewport );', 1),
    ('FString Command = FString::Printf(TEXT("%s Speed=%f"), *Bindings[iKey], Length);',
     'TCHAR InputCmd[1024]; appSprintf(InputCmd, TEXT("%s Speed=%f"), *Bindings[iKey], Length);', 1),
    ('ExecInputCommands( *Command, *GLog );', 'ExecInputCommands( InputCmd, *GLog );', 1),
    ('P_GET_BYTE(Key);', 'BYTE Key = InputArgByte();', 1),
    ('P_FINISH;', '', 1),
]
for old, new, count in repls:
    assert s.count(old) == count, (old, s.count(old))
    s = s.replace(old, new)
p.write_bytes(s.encode('utf-8'))
PY
FLAGS=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Engine/Src/UnIn.cpp "$REPO" "$WORK")
FLAGS_MISC=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Core/Src/UnMisc.cpp "$REPO" "$WORK")
FLAGS_ANSI=$(python3 "$REPO/Build/fixture_common.py" flags "$CC_DB" HarryPotter2/Unreal/Core/Src/UnAnsi.cpp "$REPO" "$WORK")
UNMISC=$(find "$WORK/HarryPotter2" -name UnMisc.cpp | head -1)
UNANSI=$(find "$WORK/HarryPotter2" -name UnAnsi.cpp | head -1)
cd "$WORK/HarryPotter2/Unreal/Engine/Src"
eval "c++ $FLAGS -O3 -g -c UnIn.cpp -o $WORK/UnIn.o"
eval "c++ $FLAGS_MISC -O3 -g -c $UNMISC -o $WORK/UnMisc.o"
eval "c++ $FLAGS_ANSI -O3 -g -c $UNANSI -o $WORK/UnAnsi.o"
eval "c++ $FLAGS -O3 -g -fno-access-control -c $REPO/Build/input_fixture_harness/input_harness.cpp -o $WORK/harness.o"
OBJS="$WORK/UnIn.o $WORK/UnMisc.o $WORK/UnAnsi.o $WORK/harness.o"
{ for o in $OBJS; do nm -g --defined-only "$o"; done; } \
  | awk '{print $NF}' | sed 's/^_//' | sort -u > "$WORK/defined.txt"
{ for o in $OBJS; do nm -u "$o"; done; } | awk '{print $NF}' | sed 's/^_//' \
  | grep -v -E '^(setjmp|__cxa_|__gxx|__stack_chk|__chkstk|_Unwind|_ZSt9|_ZTVN10__cxx|_ZTIPc|_ZTIi)' \
  | grep -v -E '^(bzero|mem[a-z]+|memset_pattern[0-9]+|str[a-z]+|v?snprintf|v?sprintf|v?fprintf|printf|malloc|calloc|realloc|free|abort|exit|fabs|sqrt|floor|ceil|fmod|pow|log|exp|sin|cos|tan|atan2?|acos|asin|rand|srand|time|clock|wcs[a-z]+|wcsn[a-z]+|atof|putchar)$' \
  | sort -u | comm -23 - "$WORK/defined.txt" > "$WORK/syms.txt"
# Stubs: functions abort with STUB; data is zero-filled char[64] (bss-equivalent). Any stub hit fails the run.
python3 "$REPO/Build/fixture_common.py" stubs "$WORK/syms.txt" $OBJS > "$WORK/stubs.cpp"
c++ -c "$WORK/stubs.cpp" -o "$WORK/stubs.o"
c++ "$WORK/harness.o" "$WORK/UnIn.o" "$WORK/UnMisc.o" "$WORK/UnAnsi.o" "$WORK/stubs.o" -o "$WORK/input_harness"
OUT=$REPO/crates/hp2-engine/tests/fixtures/input_oracle.txt
run() {
  "$1" > "$2" 2> "$3" || { cat "$3" >&2; exit 1; }
  if grep -q '^STUB ' "$3"; then cat "$3" >&2; exit 1; fi
}
run "$WORK/input_harness" "$WORK/run1.txt" "$WORK/err1.txt"
run "$WORK/input_harness" "$WORK/run2.txt" "$WORK/err2.txt"
cmp "$WORK/run1.txt" "$WORK/run2.txt"
test "$(grep -c '^call' "$WORK/run1.txt")" -eq 121
cp "$WORK/run1.txt" "$OUT"
printf 'wrote %s lines\n' "$(wc -l < "$OUT")"
