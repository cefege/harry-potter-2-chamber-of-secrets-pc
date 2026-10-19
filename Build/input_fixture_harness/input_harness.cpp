// Original-object input fixture harness. gen_input.sh rewrites the engine
// calls of the pinned UnIn.cpp to the InputFixture hooks defined here; the
// dispatch, branch, and float math run unmodified from the original object
// (UnIn.o) and the original UnMisc.o parsers.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdarg>
#include <cwchar>
#include <string>
#define private public
#define protected public
#include "EnginePrivate.h"
#undef private
#undef protected

// Core globals referenced by the original object.
UBOOL GIsRunning = 1;

// Engine allocator for the original object's operator new / FString storage. Core.cpp is not linked.
class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { (void)Tag; return malloc( Count ? Count : 1 ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { (void)Tag; return realloc( Original, Count ? Count : 1 ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

static void pw(const wchar_t* s) {
	for (; s && *s; ++s) putchar(*s < 128 ? (int)*s : '?');
}

static void emit_log(const wchar_t* s) {
	printf("log\t");
	pw(s);
	printf("\n");
}

struct InputSink : FOutputDevice {
	void Serialize(const TCHAR* S, EName) { emit_log(S); }
};
static InputSink g_sink;
FOutputDevice* GLog = &g_sink;


// Core string and memory helpers (appStrlen, appStrncpy, appStrncat, appStricmp, appStrtoi, appSprintf,
// appMemcpy, appMemmove, appMemset, ...) come from the real HarryPotter2/Unreal/Core/Src/UnAnsi.cpp,
// compiled into the harness by gen_input.sh.

// Engine objects. UInput's vtable comes from the original UnIn.o.
extern char ZTV_UInput[] __asm__("__ZTV6UInput");

static const INT kBase = (INT)sizeof(APlayerPawn);
static const INT kDump = 32;
alignas(16) static char g_pawn_mem[sizeof(APlayerPawn) + 32];
static APlayerPawn* const g_pawn = (APlayerPawn*)g_pawn_mem;
static AActor* const g_actor = (AActor*)g_pawn_mem;
alignas(16) static char g_cls_mem[2][sizeof(UClass)];
static UClass* const g_clsA = (UClass*)g_cls_mem[0];
static UClass* const g_clsB = (UClass*)g_cls_mem[1];
alignas(16) static char g_vp_mem[sizeof(UViewport)];
static UViewport* const g_vp = (UViewport*)g_vp_mem;
alignas(16) static char g_in_mem[sizeof(UInput)];
static UInput* const g_in = (UInput*)g_in_mem;
alignas(16) static char g_other_mem[sizeof(UPlayer)];
static UPlayer* const g_other = (UPlayer*)g_other_mem;
alignas(16) static char g_frame_mem[sizeof(FFrame)];

static int g_pawn_flag;
static int g_arg_key;

// Name table: index is the EName value. Keys 0..11 are IK_A..IK_L.
static const wchar_t* const kNames[] = {
	L"None", L"Fire", L"Jump", L"Turn", L"Strafe", L"Look", L"Stance", L"Hidden",
	L"Walk", L"Sprint", L"Cycle",
	L"IK_A", L"IK_B", L"IK_C", L"IK_D", L"IK_E", L"IK_F", L"IK_G", L"IK_H", L"IK_I",
	L"IK_J", L"IK_K", L"IK_L",
};
static const int kNameCount = (int)(sizeof(kNames) / sizeof(kNames[0]));

static int key_name_index(int Key) { return Key < 12 ? 11 + Key : 0; }

// Properties: class 1 = A, class 2 = B. Offsets are relative to kBase.
struct PropDef {
	int cls;
	int name;
	int kind; // 1 = byte, 2 = float
	int off;
	QWORD flags;
};
static const PropDef kProps[] = {
	{1, 1, 1, 0, (QWORD)CPF_Input},   // A Fire (byte)
	{1, 2, 1, 1, (QWORD)CPF_Input},   // A Jump (byte)
	{1, 7, 1, 2, 0},                  // A Hidden (byte, no input)
	{1, 3, 2, 4, (QWORD)CPF_Input},   // A Turn (float)
	{1, 4, 2, 8, (QWORD)CPF_Input},   // A Strafe (float)
	{1, 5, 2, 12, (QWORD)CPF_Input},  // A Look (float)
	{1, 6, 2, 16, 0},                 // A Stance (float, no input)
	{2, 8, 1, 20, (QWORD)CPF_Input},  // B Walk (byte)
	{2, 3, 2, 24, 0},                 // B Turn (float, no input)
};
static const int kPropCount = (int)(sizeof(kProps) / sizeof(kProps[0]));
alignas(16) static char g_prop_mem[kPropCount][sizeof(UFloatProperty)];
static UProperty* prop_at(int i) { return (UProperty*)g_prop_mem[i]; }

static int class_of(const UClass* C) {
	if (C == g_clsA) return 1;
	if (C == g_clsB) return 2;
	return 0;
}

static int kind_of(const UProperty* P) {
	for (int i = 0; i < kPropCount; i++)
		if (prop_at(i) == P) return kProps[i].kind;
	return 0;
}

static const char* cls_name(const UClass* C) {
	if (C == g_clsA) return "A";
	if (C == g_clsB) return "B";
	return "-";
}

static const wchar_t* const kAliasCommands[2] = {
	L"BUTTON Fire|AXIS Turn SPEED=2",
	L"TOGGLE Jump",
};
static const int kAliasNames[2] = {9, 10};

static const wchar_t* const kBindings[12] = {
	L"BUTTON Fire",
	L"PULSE Jump|AXIS Turn SPEED=2",
	L"AXIS Strafe SPEEDBASE=3 SPEED=0.5",
	L"AXIS Look",
	L"TOGGLE Fire",
	L"Sprint",
	L"KEYNAME 3",
	L"",
	L"OnRelease BUTTON Fire|BUTTON Hidden",
	L"ZZZ",
	L"BUTTON \"Jump\"",
	L"BUTTON Walk",
};

static void set_fstr(FString& S, const wchar_t* Text) {
	INT N = (INT)wcslen(Text) + 1;
	TCHAR* D = (TCHAR*)malloc(sizeof(TCHAR) * N);
	memcpy(D, Text, sizeof(TCHAR) * N);
	S.Data = D;
	S.ArrayNum = N;
	S.ArrayMax = N;
}

// Oracle hooks referenced by the patched UnIn.cpp.
UProperty* InputNthField(UClass* Class, INT Kind, INT N) {
	int C = class_of(Class);
	if (!C) return NULL;
	int Seen = 0;
	for (int i = 0; i < kPropCount; i++) {
		if (kProps[i].cls != C) continue;
		if (Kind && kProps[i].kind != Kind) continue;
		if (Seen++ == N) return prop_at(i);
	}
	return NULL;
}

struct CacheEnt {
	QWORD Id;
	void* Mem;
};
static CacheEnt g_cache[4];
static int g_ncache;
alignas(16) static char g_item_mem[16];

void* InputCacheGet(QWORD CacheId, FMemCache::FCacheItem*& Item) {
	Item = (FMemCache::FCacheItem*)g_item_mem;
	for (int i = 0; i < g_ncache; i++)
		if (g_cache[i].Id == CacheId) return g_cache[i].Mem;
	return NULL;
}

void* InputCacheCreate(QWORD CacheId, FMemCache::FCacheItem*& Item, SIZE_T Size) {
	Item = (FMemCache::FCacheItem*)g_item_mem;
	printf("event\tcache_create\tsize=%zu\n", (size_t)Size);
	void* Mem = calloc(1, Size);
	g_cache[g_ncache].Id = CacheId;
	g_cache[g_ncache].Mem = Mem;
	g_ncache++;
	return Mem;
}

void InputCacheUnlock(FMemCache::FCacheItem* Item) { (void)Item; }

EName InputFindName(const TCHAR* Text) {
	for (int i = 0; i < kNameCount; i++)
		if (!wcscmp(kNames[i], Text)) return (EName)i;
	return NAME_None;
}

const TCHAR* InputKeyChars(INT Key) { return kNames[key_name_index(Key)]; }

UBOOL InputKeyFindItem(FName N, INT& Index) {
	for (int k = 0; k < IK_MAX; k++) {
		if (key_name_index(k) == N.GetIndex()) {
			Index = k;
			return 1;
		}
	}
	return 0;
}

UByteProperty* InputCastByte(UProperty* P) { return kind_of(P) == 1 ? (UByteProperty*)P : NULL; }
UFloatProperty* InputCastFloat(UProperty* P) { return kind_of(P) == 2 ? (UFloatProperty*)P : NULL; }
APlayerPawn* InputCastPawn(AActor* A) { return (g_pawn_flag && A == g_actor) ? (APlayerPawn*)A : NULL; }
UViewport* InputCastViewport(UPlayer* P) { return (P == (UPlayer*)g_vp) ? g_vp : NULL; }

void InputViewportExec(UViewport* V, const TCHAR* Str, FOutputDevice& Ar) {
	(void)V;
	(void)Ar;
	printf("event\tviewport_exec\t");
	pw(Str);
	printf("\n");
}

void InputViewportUpdate(UViewport* V, INT Flag) {
	(void)V;
	printf("event\tviewport_update\t%d\n", (int)Flag);
}

void InputLogInit(UViewport* V) {
	(void)V;
	printf("event\tinit_log\n");
}

BYTE InputArgByte() { return (BYTE)g_arg_key; }

// Recording state.
struct St {
	int act;
	float delta;
	int running;
	int actor;
	int cls;
	int pawn;
	int player;
	int input;
	uint8_t mem[32];
	uint8_t keys[IK_MAX];
	St() : act(0), delta(0.f), running(1), actor(1), cls(1), pawn(0), player(0), input(1) {
		memset(mem, 0, sizeof(mem));
		memset(keys, 0, sizeof(keys));
	}
};

static void setb(St& S, int Off, int V) { S.mem[Off] = (uint8_t)V; }
static void setf(St& S, int Off, float V) { memcpy(&S.mem[Off], &V, 4); }

static uint32_t fbits(float F) {
	uint32_t U;
	memcpy(&U, &F, 4);
	return U;
}

static void apply(const St& S) {
	g_pawn_flag = S.pawn;
	GIsRunning = S.running;
	g_vp->Actor = S.actor ? g_pawn : NULL;
	g_vp->Input = S.input ? g_in : NULL;
	g_actor->Class = S.cls == 1 ? g_clsA : (S.cls == 2 ? g_clsB : NULL);
	memcpy(g_pawn_mem + kBase, S.mem, kDump);
	g_pawn->Player = S.player == 1 ? (UPlayer*)g_vp : (S.player == 2 ? g_other : NULL);
	g_in->Viewport = g_vp;
	g_in->Action = (EInputAction)S.act;
	g_in->Delta = S.delta;
	memcpy(g_in->KeyDownTable, S.keys, IK_MAX);
}

static void dump(const char* Tag) {
	printf("%s\tact=%d\tdelta=%08x\trunning=%d\tactor=%d\tclass=%s\tmem=", Tag, (int)g_in->Action,
		(unsigned)fbits(g_in->Delta), (int)GIsRunning, g_vp->Actor ? 1 : 0, cls_name(g_actor->Class));
	for (INT i = 0; i < kDump; i++) printf("%02x", (unsigned)(uint8_t)g_pawn_mem[kBase + i]);
	printf("\tkeys=");
	bool Any = false;
	for (int k = 0; k < IK_MAX; k++) {
		if (g_in->KeyDownTable[k]) {
			printf("%s%d:%d", Any ? "," : "", k, (int)g_in->KeyDownTable[k]);
			Any = true;
		}
	}
	if (!Any) printf("-");
	printf("\n");
}

static std::string W(const wchar_t* S) {
	std::string R;
	for (; S && *S; ++S) R += (char)(*S < 128 ? *S : '?');
	return R;
}

static std::string I(long long V) { return std::to_string(V); }

static void begin(const char* Op, const std::string& Args, const St& S) {
	apply(S);
	g_ncache = 0;
	printf("call\top=%s%s\n", Op, Args.c_str());
	dump("pre");
}

static void finish(const std::string& Ret) {
	printf("ret\t%s\n", Ret.c_str());
	dump("post");
}

static void x_exec(const St& S, const wchar_t* Cmd) {
	begin("exec", "\tcmd=" + W(Cmd), S);
	UBOOL R = g_in->Exec(Cmd, *GLog);
	finish(I(R));
}

static void x_eic(const St& S, const std::wstring& Cmd) {
	begin("exec_commands", "\tcmd=" + W(Cmd.c_str()), S);
	g_in->ExecInputCommands(Cmd.c_str(), *GLog);
	finish("-");
}

static void x_process(const St& S, int Key, int Act, float Delta) {
	begin("process", "\tkey=" + I(Key) + "\tact=" + I(Act) + "\tdelta=" + I(fbits(Delta)), S);
	UBOOL R = g_in->Process(*GLog, (EInputKey)Key, (EInputAction)Act, Delta);
	finish(I(R));
}

static void x_preprocess(const St& S, int Key, int Act, float Delta) {
	begin("preprocess", "\tkey=" + I(Key) + "\tact=" + I(Act) + "\tdelta=" + I(fbits(Delta)), S);
	UBOOL R = g_in->PreProcess((EInputKey)Key, (EInputAction)Act, Delta);
	finish(I(R));
}

static void x_direct(const St& S, int Key, float Length, float Delta) {
	begin("direct_axis", "\tkey=" + I(Key) + "\tlength=" + I(fbits(Length)) + "\tdelta=" + I(fbits(Delta)), S);
	g_in->DirectAxis((EInputKey)Key, Length, Delta);
	finish("-");
}

static void x_read(const St& S, float Delta) {
	begin("read_input", "\tdelta=" + I(fbits(Delta)), S);
	g_in->ReadInput(Delta, *GLog);
	finish("-");
}

static void x_reset(const St& S) {
	begin("reset_input", "", S);
	g_in->ResetInput();
	finish("-");
}

static void x_init(const St& S) {
	begin("init", "", S);
	g_in->Init(g_vp);
	finish("-");
}

static void x_button(const St& S, const wchar_t* Name) {
	begin("find_button", "\tname=" + W(Name), S);
	BYTE* P = g_in->FindButtonName(g_actor, Name);
	finish(P ? "off=" + I((char*)P - (char*)g_actor) : std::string("none"));
}

static void x_axis(const St& S, const wchar_t* Name) {
	begin("find_axis", "\tname=" + W(Name), S);
	FLOAT* P = g_in->FindAxisName(g_actor, Name);
	finish(P ? "off=" + I((char*)P - (char*)g_actor) : std::string("none"));
}

static void x_keyname(const St& S, int Key) {
	begin("get_key_name", "\tkey=" + I(Key), S);
	const TCHAR* T = g_in->GetKeyName((EInputKey)Key);
	finish("text=" + W(T));
}

static void x_findkey(const St& S, const wchar_t* Name) {
	begin("find_key_name", "\tname=" + W(Name), S);
	EInputKey K = (EInputKey)-1;
	int R = g_in->FindKeyName(Name, K);
	finish(R ? "1\tkey=" + I((int)K) : std::string("0"));
}

static void x_keystate(const St& S, int Key) {
	g_arg_key = Key;
	begin("key_state", "\tkey=" + I(Key) + "\tpawn=" + I(S.pawn) + "\tplayer=" + I(S.player) +
		"\tinput=" + I(S.input), S);
	alignas(16) BYTE Result[16];
	memset(Result, 0, sizeof(Result));
	g_actor->execGetCurrentKeyState(*(FFrame*)g_frame_mem, Result);
	finish(I(*(UBOOL*)Result));
}

static void print_header() {
	printf("header\tversion=1\tmem_base=%d\tmem_size=%d\tik_max=%d\talias_max=%d\n", (int)kBase, (int)kDump,
		(int)IK_MAX, (int)ALIAS_MAX);
	for (int i = 0; i < kNameCount; i++) {
		printf("name\tindex=%d\ttext=", i);
		pw(kNames[i]);
		printf("\n");
	}
	for (int k = 0; k < IK_MAX; k++) {
		printf("key\tindex=%d\tname_index=%d\ttext=", k, key_name_index(k));
		pw(InputKeyChars(k));
		printf("\n");
	}
	printf("class\tname=A\tid=1\nclass\tname=B\tid=2\n");
	for (int i = 0; i < kPropCount; i++) {
		printf("prop\tclass=%s\tindex=%d\tname_index=%d\tkind=%s\toffset=%d\tflags=%llx\n",
			kProps[i].cls == 1 ? "A" : "B", i, kProps[i].name, kProps[i].kind == 1 ? "byte" : "float",
			(int)(kBase + kProps[i].off), (unsigned long long)kProps[i].flags);
	}
	for (int i = 0; i < ALIAS_MAX; i++) {
		if (i < 2) {
			printf("alias\tindex=%d\tname_index=%d\tcommand=", i, kAliasNames[i]);
			pw(kAliasCommands[i]);
			printf("\n");
		} else {
			printf("alias\tindex=%d\tname_index=0\tcommand=\n", i);
		}
	}
	for (int k = 0; k < IK_MAX; k++) {
		printf("binding\tindex=%d\ttext=", k);
		if (k < 12) pw(kBindings[k]);
		printf("\n");
	}
}

int main() {
	// Object setup: UInput gets the original vtable, FStrings get fixed buffers.
	*(void**)g_in_mem = (void*)(ZTV_UInput + 16);
	for (int i = 0; i < kPropCount; i++) {
		UProperty* P = prop_at(i);
		P->Name = FName((EName)kProps[i].name);
		P->Offset = kBase + kProps[i].off;
		P->PropertyFlags = (DWORD)kProps[i].flags;
	}
	for (int i = 0; i < ALIAS_MAX; i++) {
		if (i < 2) {
			g_in->Aliases[i].Alias = FName((EName)kAliasNames[i]);
			set_fstr(g_in->Aliases[i].Command, kAliasCommands[i]);
		} else {
			set_fstr(g_in->Aliases[i].Command, L"");
		}
	}
	for (int k = 0; k < IK_MAX; k++) set_fstr(g_in->Bindings[k], k < 12 ? kBindings[k] : L"");
	g_in->InputKeys = NULL;
	g_vp->Input = g_in;
	print_header();

	// Exec: button, pulse, toggle, axis, keyname, keybinding, alias, unknown.
	St a;
	a.act = IST_Press;
	x_exec(a, L"BUTTON Fire");
	a.act = IST_Release; setb(a, 0, 0x80);
	x_exec(a, L"BUTTON Fire");
	a.act = IST_Hold; setb(a, 1, 0);
	x_exec(a, L"BUTTON Jump");
	a.act = IST_Press;
	x_exec(a, L"BUTTON Nope");
	{ St s = a; s.actor = 0; x_exec(s, L"BUTTON Fire"); }
	x_exec(a, L"BUTTON");
	x_exec(a, L"BUTTON Hidden");
	x_exec(a, L"BUTTON Walk");
	{ St s = a; s.cls = 2; setb(s, 20, 0); x_exec(s, L"BUTTON Walk"); }
	x_exec(a, L"BUTTON \"Fire\"");
	a.act = IST_Press; setb(a, 1, 0);
	x_exec(a, L"PULSE Jump");
	a.act = IST_Release;
	x_exec(a, L"PULSE Jump");
	a.act = IST_Press;
	x_exec(a, L"PULSE Nope");
	x_exec(a, L"PULSE");
	a.act = IST_Press; setb(a, 0, 0);
	x_exec(a, L"TOGGLE Fire");
	a.act = IST_Release;
	x_exec(a, L"TOGGLE Fire");
	x_exec(a, L"TOGGLE Nope");
	a.act = IST_Axis; a.delta = 0.5f; setf(a, 4, 1.0f);
	x_exec(a, L"AXIS Turn SPEED=2");
	a.act = IST_Hold;
	x_exec(a, L"AXIS Turn SPEEDBASE=3 SPEED=2");
	x_exec(a, L"AXIS Strafe SPEEDBASE=0.5");
	a.act = IST_Axis;
	x_exec(a, L"AXIS Strafe speed=abc");
	a.act = IST_Press;
	x_exec(a, L"AXIS Look");
	a.act = IST_Axis;
	x_exec(a, L"AXIS Stance");
	{ St s = a; s.actor = 0; x_exec(s, L"AXIS Look"); }
	x_exec(a, L"AXIS");
	a.act = IST_Press;
	x_exec(a, L"KEYNAME 3");
	x_exec(a, L"KEYNAME 20");
	x_exec(a, L"KEYNAME -1");
	x_exec(a, L"KEYNAME 255");
	x_exec(a, L"KEYNAME 0");
	x_exec(a, L"KEYNAME 11");
	x_exec(a, L"KEYBINDING A");
	x_exec(a, L"KEYBINDING B");
	x_exec(a, L"KEYBINDING Q");
	x_exec(a, L"KEYBINDING Z");
	x_exec(a, L"KEYBINDING L");
	x_exec(a, L"KEYBINDING ");
	x_exec(a, L"Sprint");
	x_exec(a, L"Cycle");
	x_exec(a, L"Nope");
	x_exec(a, L"");
	x_exec(a, L"   ");
	x_exec(a, L"Sprint extra");

	// ExecInputCommands: line splitting, OnRelease, comments, viewport fallback.
	a.act = IST_Press;
	x_eic(a, L"BUTTON Fire|PULSE Jump");
	a.act = IST_Release; setb(a, 0, 0x80);
	x_eic(a, L"OnRelease BUTTON Fire|BUTTON Jump");
	x_eic(a, L"BUTTON Fire|OnRelease PULSE Jump");
	a.act = IST_Press;
	x_eic(a, L"// comment|BUTTON Fire");
	x_eic(a, L"BUTTON Fire // x");
	x_eic(a, L"Some viewport command");
	x_eic(a, std::wstring(300, L'v'));
	a.act = IST_Hold;
	x_eic(a, L"Sprint");
	x_eic(a, L"");

	// Process: binding dispatch with action and delta, then SetInputAction(None).
	a = St();
	a.act = IST_None;
	x_process(a, 0, IST_Press, 0.5f);
	x_process(a, 2, IST_Press, 0.25f);
	x_process(a, 1, IST_Hold, 0.1f);
	x_process(a, 5, IST_Press, 0.f);
	x_process(a, 8, IST_Release, 0.f);
	x_process(a, 7, IST_Press, 0.f);
	x_process(a, 9, IST_Press, 0.f);
	x_process(a, 12, IST_Press, 0.f);
	x_process(a, 254, IST_Press, 0.f);

	// PreProcess: key table transitions.
	{ St s = a; x_preprocess(s, 3, IST_Press, 0.f); }
	{ St s = a; s.keys[3] = 1; x_preprocess(s, 3, IST_Press, 0.f); }
	{ St s = a; s.keys[3] = 1; x_preprocess(s, 3, IST_Release, 0.f); }
	{ St s = a; x_preprocess(s, 3, IST_Release, 0.f); }
	{ St s = a; s.keys[3] = 1; x_preprocess(s, 3, IST_Hold, 0.5f); }
	{ St s = a; x_preprocess(s, 3, IST_None, 0.f); }

	// DirectAxis: formatted command with Speed= appended.
	x_direct(a, 2, 0.5f, 0.25f);
	x_direct(a, 0, 1.f, 0.f);
	x_direct(a, 7, 1.f, 0.5f);
	x_direct(a, 3, -2.5f, 0.f);
	x_direct(a, 1, 0.125f, 0.75f);

	// ReadInput: hold processing and axis scaling (20 / delta).
	{ St s; setb(s, 0, 0); setf(s, 4, 1.0f); setf(s, 8, 2.0f); setf(s, 12, -3.0f); setf(s, 16, 4.0f);
		s.keys[0] = 1; x_read(s, 0.5f); }
	{ St s; setf(s, 4, 1.0f); setf(s, 8, 2.0f); setf(s, 12, -3.0f); setf(s, 16, 4.0f);
		x_read(s, -1.f); }
	{ St s; setf(s, 4, 1.0f); setf(s, 8, 2.0f); setf(s, 12, -3.0f); setf(s, 16, 4.0f);
		x_read(s, 0.f); }
	{ St s; setf(s, 4, 1.0f); s.running = 0; x_read(s, 0.5f); }
	{ St s; s.cls = 2; setb(s, 20, 1); setf(s, 24, 2.0f); x_read(s, 0.25f); }
	{ St s; s.keys[1] = 1; s.keys[2] = 1; setf(s, 4, 1.0f); x_read(s, 0.1f); }

	// ResetInput and Init.
	{ St s; s.act = IST_Hold; s.delta = 0.5f; s.keys[3] = 1; s.keys[200] = 1;
		setb(s, 0, 1); setb(s, 1, 1); setb(s, 2, 1); setf(s, 4, 1.f); setf(s, 8, 2.f);
		setf(s, 12, 3.f); setf(s, 16, 4.f); x_reset(s); }
	{ St s; setb(s, 20, 1); setf(s, 24, 5.f); s.cls = 2; x_reset(s); }
	{ St s; s.keys[3] = 1; setb(s, 0, 1); setf(s, 4, 2.f); x_init(s); }

	// Name lookups.
	x_keyname(a, 0);
	x_keyname(a, 3);
	x_keyname(a, 11);
	x_keyname(a, 12);
	x_keyname(a, 254);
	x_keyname(a, 255);
	x_keyname(a, -1);
	x_findkey(a, L"A");
	x_findkey(a, L"K");
	x_findkey(a, L"L");
	x_findkey(a, L"Z");
	x_findkey(a, L"l");
	x_findkey(a, L"");
	x_findkey(a, L"IK_A");

	// FindButtonName / FindAxisName (cache creation included).
	x_button(a, L"Fire");
	x_button(a, L"Jump");
	x_button(a, L"Hidden");
	x_button(a, L"Turn");
	x_button(a, L"Walk");
	{ St s = a; s.cls = 2; x_button(s, L"Walk"); x_button(s, L"Fire"); }
	x_button(a, L"Nope");
	x_axis(a, L"Turn");
	x_axis(a, L"Strafe");
	x_axis(a, L"Look");
	x_axis(a, L"Stance");
	x_axis(a, L"Fire");
	{ St s = a; s.cls = 2; x_axis(s, L"Turn"); x_axis(s, L"Walk"); }

	// execGetCurrentKeyState: pawn cast, player viewport, input, key range.
	{ St s; s.pawn = 1; s.player = 1; s.keys[3] = 1; x_keystate(s, 3); }
	{ St s; s.pawn = 1; s.player = 1; x_keystate(s, 3); }
	{ St s; s.pawn = 0; s.player = 1; s.keys[3] = 1; x_keystate(s, 3); }
	{ St s; s.pawn = 1; s.player = 2; s.keys[3] = 1; x_keystate(s, 3); }
	{ St s; s.pawn = 1; s.player = 0; s.keys[3] = 1; x_keystate(s, 3); }
	{ St s; s.pawn = 1; s.player = 1; s.input = 0; s.keys[3] = 1; x_keystate(s, 3); }
	{ St s; s.pawn = 1; s.player = 1; s.keys[254] = 1; x_keystate(s, 254); }
	{ St s; s.pawn = 1; s.player = 1; s.keys[254] = 1; x_keystate(s, 255); }

	// Axis scaling inputs where mul_add and the unfused product differ (checked in Rust test).
	{ St s; s.act = IST_Axis; s.delta = 8.7f; setf(s, 4, 0.2f); x_exec(s, L"AXIS Turn SPEEDBASE=1.15 SPEED=0.13"); }
	{ St s; s.act = IST_Axis; s.delta = 0.39f; setf(s, 4, 0.185f); x_exec(s, L"AXIS Turn SPEED=0.55"); }
	{ St s; s.act = IST_Hold; s.delta = 0.1f; setf(s, 4, 0.26f); x_exec(s, L"AXIS Turn SPEED=0.15"); }
	return 0;
}
