// Original-object UnEngine fixture harness. gen_uengine.sh rewrites the engine
// calls of the pinned UnEngine.cpp (6bd3da5) to the hooks defined here; the
// dispatch, branch, guard and float math run unmodified from the original
// object (UnEngine.o) and the original UnMisc.o / UnAnsi.o helpers
// (ParseCommand, appStricmp). Every virtual UEngine entry point is called
// qualified because the harness engine memory has no vtable; the port's
// virtual dispatch is exercised through the services adapters instead.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <initializer_list>
#define private public
#define protected public
#include "EnginePrivate.h"
#undef private
#undef protected

// Core globals referenced by the original object.
UBOOL GIsRunning = 1;
UBOOL GIsClient = 1;
UBOOL GIsEditor = 0;
UBOOL GIsRequestingExit = 0;
USystem* GSys = NULL;
FExec* GExec = NULL;

// Engine allocator for the original object's FString storage. Core.cpp is not linked.
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

// The harness passes *GLog as the output device of the oracle; log text is
// recorded at the hook that receives it, so the sink itself emits nothing.
struct EngSink : FOutputDevice {
	void Serialize(const TCHAR* S, EName) { (void)S; }
};
static EngSink g_sink;
FOutputDevice* GLog = &g_sink;

static unsigned fbits(FLOAT v) {
	unsigned b;
	memcpy(&b, &v, 4);
	return b;
}

// Opaque engine objects. Only their identity matters to the oracle; the
// viewport, pawn and engine are typed so member accesses compile.
enum {
	FK_AUDIO_CLASS = 0, FK_AUDIO = 1, FK_RENDER = 2, FK_CLIENT = 3,
	FK_CYLINDER = 4, FK_OCYLINDER = 5, FK_OVAL = 6, FK_OOVAL = 7, FK_BOX = 8,
	FK_SYS = 9, FK_EXEC = 10, FK_CONSOLE = 11, FK_INPUT = 12, FK_MENU = 13,
	FK_URL = 14, FK_ENGINE_CLASS = 15, FK_HUD = 16,
};
alignas(16) static char g_fake[32][256];
static void* fake(int i) { return g_fake[i]; }

alignas(16) static char g_engine_mem[sizeof(UEngine)];
alignas(16) static char g_cmdlet_mem[sizeof(UServerCommandlet)];
alignas(16) static char g_vp_mem[sizeof(UViewport)];
alignas(16) static char g_pawn_mem[sizeof(APlayerPawn)];
static UEngine* const g_eng = (UEngine*)g_engine_mem;
static UServerCommandlet* const g_cmdlet = (UServerCommandlet*)g_cmdlet_mem;
static UViewport* const g_vp = (UViewport*)g_vp_mem;
static APlayerPawn* const g_pawn = (APlayerPawn*)g_pawn_mem;

// Class chain for MainMenu->GetClass() / GetSuperClass() / GetName().
struct FakeClass { const TCHAR* Name; FakeClass* Super; };
static FakeClass* g_class_head = NULL;

static void set_chain(std::initializer_list<const TCHAR*> names) {
	static FakeClass pool[4];
	FakeClass* prev = NULL;
	g_class_head = NULL;
	int i = 0;
	for (const TCHAR* n : names) {
		pool[i].Name = n;
		pool[i].Super = NULL;
		if (prev) prev->Super = &pool[i];
		else g_class_head = &pool[i];
		prev = &pool[i];
		i++;
	}
}

// Scripted answers for the hooks below.
struct Script {
	UBOOL nosound = 0;
	UBOOL audio_init = 1;
	UBOOL config_lang = 1;
	UBOOL key_type = 0;
	UBOOL key_event = 0;
	UBOOL pre = 1;
	UBOOL proc = 1;
	UBOOL url_valid = 1;
	INT claim = -1;
};
static Script g_s;
static INT g_ticks = 0;
static INT g_clock_idx = 0;

// appSeconds script: exact multiples of 0.25 s (2^32 raw units each).
static const double kClockSecs[] = {
	0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.25, 3.5, 3.75, 4.0,
};
static const INT kTickStop = 6;

static const TCHAR kYes[] = TEXT("yes");
static const TCHAR kNo[] = TEXT("no");
static const TCHAR kUrl[] = TEXT("unreal");
static const TCHAR kHost[] = TEXT("127.0.0.1");
static const TCHAR kMap[] = TEXT("Entry");
static const TCHAR kPortal[] = TEXT("Portal");
static const TCHAR kOpt0[] = TEXT("?Game=x");
static const TCHAR kOpt1[] = TEXT("?Listen");
static const TCHAR kUrlString[] = TEXT("unreal://127.0.0.1:7777/Entry?Game=x?Listen#Portal");

// ---- Hooks (signatures match the prelude in gen_uengine.sh) ----

UBOOL EngParseParam(const TCHAR* Param) { printf("parse_param\t"); pw(Param); printf("\n"); return g_s.nosound; }
void* EngLoadAudioClass() { printf("load_audio_class\n"); return fake(FK_AUDIO_CLASS); }
void* EngConstructAudio(void* Class) { (void)Class; printf("construct_audio\n"); return fake(FK_AUDIO); }
UBOOL EngAudioInit(void* Audio) { (void)Audio; printf("audio_init\n"); return g_s.audio_init; }
void EngLogAudioInitFailed() { printf("log_audio_init_failed\n"); }
void EngAudioDelete(void* Audio) { (void)Audio; printf("audio_delete\n"); }
void EngEngineMemInit(INT Bytes, const TCHAR* Name) { printf("engine_mem_init\t%d\t", Bytes); pw(Name); printf("\n"); }
void EngEngineMemExit() { printf("engine_mem_exit\n"); }
void EngCacheInit(INT Bytes, INT Align) { printf("cache_init\t%d\t%d\n", Bytes, Align); }
void EngCacheExit(INT FreeMemory) { printf("cache_exit\t%d\n", FreeMemory); }
void EngCacheFlush() { printf("gcache_flush\n"); }
UBOOL EngCacheExec(const TCHAR* Cmd, FOutputDevice& Ar) { (void)Ar; printf("gcache_exec\t"); pw(Cmd); printf("\n"); return 0; }
void EngUrlStaticInit() { printf("url_static_init\n"); }
void EngUrlStaticExit() { printf("url_static_exit\n"); }
const TCHAR* EngLocalize(const char* Section, const char* Key, const TCHAR* Package) {
	printf("localize\t%s\t%s\t", Section, Key);
	pw(Package);
	printf("\n");
	return strcmp(Key, "Yes") == 0 ? kYes : kNo;
}
void* EngNewObject(const char* Kind) {
	printf("new_object\t%s\n", Kind);
	if (strcmp(Kind, "UCylinder") == 0) return fake(FK_CYLINDER);
	if (strcmp(Kind, "UOrientedCylinder") == 0) return fake(FK_OCYLINDER);
	if (strcmp(Kind, "UAlignedOvalCylinder") == 0) return fake(FK_OVAL);
	if (strcmp(Kind, "UOrientedOvalCylinder") == 0) return fake(FK_OOVAL);
	return fake(FK_BOX);
}
void EngAddToRoot() { printf("add_to_root\n"); }
void EngRemoveFromRoot() { printf("remove_from_root\n"); }
void EngLogEngineInitialized() { printf("log_engine_initialized\n"); }

// Exec claims: the claimer index is g_s.claim (0 sys .. 6 audio).
static UBOOL claim(INT Index) { return g_s.claim == Index; }
UBOOL EngSysExec(void* Sys, const TCHAR* Cmd, FOutputDevice& Ar) { (void)Sys; (void)Ar; printf("gsys_exec\t"); pw(Cmd); printf("\n"); return claim(0); }
UBOOL EngStaticExec(const TCHAR* Cmd, FOutputDevice& Ar) { (void)Ar; printf("static_exec\t"); pw(Cmd); printf("\n"); return claim(1); }
UBOOL EngExecExec(void* Exec, const TCHAR* Cmd, FOutputDevice& Ar) { (void)Exec; (void)Ar; printf("gexec_exec\t"); pw(Cmd); printf("\n"); return claim(3); }
UBOOL EngClientExec(void* Client, const TCHAR* Cmd, FOutputDevice& Ar) { (void)Client; (void)Ar; printf("client_exec\t"); pw(Cmd); printf("\n"); return claim(4); }
UBOOL EngRenderExec(void* Render, const TCHAR* Cmd, FOutputDevice& Ar) { (void)Render; (void)Ar; printf("render_exec\t"); pw(Cmd); printf("\n"); return claim(5); }
UBOOL EngAudioExec(void* Audio, const TCHAR* Cmd, FOutputDevice& Ar) { (void)Audio; (void)Ar; printf("audio_exec\t"); pw(Cmd); printf("\n"); return claim(6); }
void EngClientFlush(void* Client, INT AllowPrecache) { (void)Client; printf("client_flush\t%d\n", AllowPrecache); }
void EngOutLog(FOutputDevice& Ar, const TCHAR* Text) { (void)Ar; printf("log\t"); pw(Text); printf("\n"); }
void EngOutLogf(FOutputDevice& Ar, const TCHAR* Fmt) { (void)Ar; printf("logf\t"); pw(Fmt); printf("\n"); }
void EngOutLogfS(FOutputDevice& Ar, const TCHAR* Fmt, const TCHAR* S) { (void)Ar; printf("logfs\t"); pw(Fmt); printf("\t"); pw(S); printf("\n"); }
void EngOutLogfI(FOutputDevice& Ar, const TCHAR* Fmt, INT I) { (void)Ar; printf("logfi\t"); pw(Fmt); printf("\t%d\n", I); }
void EngOutLogfIS(FOutputDevice& Ar, const TCHAR* Fmt, INT I, const TCHAR* S) { (void)Ar; printf("logfis\t"); pw(Fmt); printf("\t%d\t", I); pw(S); printf("\n"); }

void* EngUrlCrack(const TCHAR* Cmd) { printf("url_crack\t"); pw(Cmd); printf("\n"); return fake(FK_URL); }
UBOOL EngUrlValid(void* URL) { (void)URL; return g_s.url_valid; }
const TCHAR* EngUrlProtocol(void* URL) { (void)URL; return kUrl; }
const TCHAR* EngUrlHost(void* URL) { (void)URL; return kHost; }
INT EngUrlPort(void* URL) { (void)URL; return 7777; }
const TCHAR* EngUrlMap(void* URL) { (void)URL; return kMap; }
INT EngUrlNumOptions(void* URL) { (void)URL; return 2; }
const TCHAR* EngUrlOption(void* URL, INT I) { (void)URL; return I == 0 ? kOpt0 : kOpt1; }
const TCHAR* EngUrlPortal(void* URL) { (void)URL; return kPortal; }
const TCHAR* EngUrlString(void* URL) { (void)URL; return kUrlString; }
void EngUrlFree(void* URL) { (void)URL; printf("url_free\n"); }

UBOOL EngConsoleEventKeyType(void* Console, INT Key) { (void)Console; printf("console_key_type\t%d\n", Key); return g_s.key_type; }
UBOOL EngConsoleEventKeyEvent(void* Console, INT Key, INT State, FLOAT Delta) {
	(void)Console;
	printf("console_key_event\t%d\t%d\t%08x\n", Key, State, fbits(Delta));
	return g_s.key_event;
}
UBOOL EngInputPreProcess(void* Input, INT Key, INT State, FLOAT Delta) {
	(void)Input;
	printf("input_pre_process\t%d\t%d\t%08x\n", Key, State, fbits(Delta));
	return g_s.pre;
}
UBOOL EngInputProcess(void* Input, FOutputDevice& Ar, INT Key, INT State, FLOAT Delta) {
	(void)Input;
	printf("input_process\t%d\t%d\t%08x\tar=%s\n", Key, State, fbits(Delta), &Ar == GLog ? "glog" : "console");
	return g_s.proc;
}
void* EngObjectClass(void* Obj) { (void)Obj; return g_class_head; }
void* EngClassSuper(void* Class) { return ((FakeClass*)Class)->Super; }
const TCHAR* EngClassName(void* Class) { return ((FakeClass*)Class)->Name; }

UBOOL EngConfigGetLanguage(TCHAR* Buf, INT Len) {
	printf("config_get_language\t%d\n", Len);
	if (g_s.config_lang) {
		Buf[0] = L'f';
		Buf[1] = L'r';
		Buf[2] = 0;
	}
	return g_s.config_lang;
}
void EngSetLanguage(const TCHAR* Text) { printf("set_language\t"); pw(Text); printf("\n"); }
void* EngLoadEngineClass() { printf("load_engine_class\n"); return fake(FK_ENGINE_CLASS); }
void* EngConstructEngine(void* Class) { (void)Class; printf("construct_engine\n"); return g_engine_mem; }
void EngEngineTick(void* Engine, FLOAT Delta) {
	(void)Engine;
	printf("engine_tick\t%08x\n", fbits(Delta));
	if (++g_ticks == kTickStop) GIsRunning = 0;
}
void EngSetCurrentTickRate(void* Engine, FLOAT Rate) {
	printf("set_current_tick_rate\t%08x\n", fbits(Rate));
	((UEngine*)Engine)->CurrentTickRate = Rate;
}
void EngAppSleep(FLOAT Seconds) { printf("app_sleep\t%08x\n", fbits(Seconds)); }
FTime EngAppSeconds() {
	double secs = kClockSecs[g_clock_idx++];
	FTime t;
	t.v = (TIMETYP)(secs * 4294967296.0);
	printf("app_seconds\t%016llx\n", (unsigned long long)t.v);
	return t;
}

// ---- Scenario driver ----

static void dump_state() {
	printf("state\tAudio=%d Render=%d Client=%d Cylinder=%d OCylinder=%d OvalCylinder=%d OOvalCylinder=%d Box=%d CurrentTickRate=%08x GIsRunning=%d\n",
		g_eng->Audio != NULL, g_eng->Render != NULL, g_eng->Client != NULL,
		g_eng->Cylinder != NULL, g_eng->OCylinder != NULL, g_eng->OvalCylinder != NULL,
		g_eng->OOvalCylinder != NULL, g_eng->Box != NULL,
		fbits(g_eng->CurrentTickRate), GIsRunning);
}

static void begin(const char* name) {
	printf("scenario\t%s\n", name);
	memset(g_engine_mem, 0, sizeof g_engine_mem);
	memset(g_cmdlet_mem, 0, sizeof g_cmdlet_mem);
	memset(g_vp_mem, 0, sizeof g_vp_mem);
	memset(g_pawn_mem, 0, sizeof g_pawn_mem);
	memset(g_fake, 0, sizeof g_fake);
	g_class_head = NULL;
	g_s = Script();
	g_ticks = 0;
	g_clock_idx = 0;
	GIsRunning = 1;
	GIsClient = 1;
	GIsEditor = 0;
	GIsRequestingExit = 0;
	GSys = (USystem*)fake(FK_SYS);
	GExec = (FExec*)fake(FK_EXEC);
	g_eng->UseSound = 1;
	g_eng->CacheSizeMegs = 5;
}

// Viewport wiring: Input is always present; Console, Actor (pawn), HUD and
// MainMenu are optional, matching the original null-check order.
static void setup_viewport(bool console, bool actor, bool hud, bool menu) {
	g_vp->Input = (decltype(g_vp->Input))fake(FK_INPUT);
	if (console) g_vp->Console = (decltype(g_vp->Console))fake(FK_CONSOLE);
	if (actor) {
		g_vp->Actor = (decltype(g_vp->Actor))g_pawn_mem;
		if (hud) {
			g_pawn->myHUD = (decltype(g_pawn->myHUD))fake(FK_HUD);
			if (menu) g_pawn->myHUD->MainMenu = (decltype(g_pawn->myHUD->MainMenu))fake(FK_MENU);
		}
	}
}

int main() {
	// Init: client and non-client cache sizing, clamping, Localize yes/no.
	begin("init_default");
	g_eng->UEngine::Init();
	dump_state();
	begin("init_no_client");
	GIsClient = 0;
	g_eng->UEngine::Init();
	dump_state();
	begin("init_clamp_high");
	g_eng->CacheSizeMegs = 5000;
	g_eng->UEngine::Init();
	dump_state();
	begin("init_clamp_low");
	g_eng->CacheSizeMegs = 0;
	g_eng->UEngine::Init();
	dump_state();

	// InitAudio.
	begin("audio_ok");
	g_eng->UEngine::InitAudio();
	dump_state();
	begin("audio_fail");
	g_s.audio_init = 0;
	g_eng->UEngine::InitAudio();
	dump_state();
	begin("audio_nosound");
	g_s.nosound = 1;
	g_eng->UEngine::InitAudio();
	dump_state();
	begin("audio_no_usesound");
	g_eng->UseSound = 0;
	g_eng->UEngine::InitAudio();
	dump_state();
	begin("audio_not_client");
	GIsClient = 0;
	g_eng->UEngine::InitAudio();
	dump_state();

	// Exit / Destroy.
	begin("exit_with_audio");
	g_eng->Audio = (decltype(g_eng->Audio))fake(FK_AUDIO);
	g_eng->UEngine::Exit();
	dump_state();
	begin("exit_without_audio");
	g_eng->UEngine::Exit();
	dump_state();
	begin("destroy");
	g_eng->Audio = (decltype(g_eng->Audio))fake(FK_AUDIO);
	g_eng->Render = (decltype(g_eng->Render))fake(FK_RENDER);
	g_eng->Client = (decltype(g_eng->Client))fake(FK_CLIENT);
	g_eng->UEngine::Destroy();
	dump_state();

	// Flush.
	begin("flush_client");
	g_eng->Client = (decltype(g_eng->Client))fake(FK_CLIENT);
	g_eng->UEngine::Flush(1);
	g_eng->UEngine::Flush(0);
	dump_state();
	begin("flush_no_client");
	g_eng->UEngine::Flush(0);
	dump_state();

	// GetMaxTickRate.
	begin("max_tick_editor");
	GIsEditor = 1;
	printf("result\tmax_tick_editor\t%08x\n", fbits(g_eng->UEngine::GetMaxTickRate()));
	dump_state();
	begin("max_tick_game");
	printf("result\tmax_tick_game\t%08x\n", fbits(g_eng->UEngine::GetMaxTickRate()));
	dump_state();

	// Exec: each claimer in chain order, then fall-through paths.
	static const char* kClaimNames[7] = { "sys", "static", "gcache", "gexec", "client", "render", "audio" };
	static const TCHAR* const kClaimCmd[7] = {
		TEXT("CLAIM sys"), TEXT("CLAIM static"), TEXT("CLAIM gcache"), TEXT("CLAIM gexec"),
		TEXT("CLAIM client"), TEXT("CLAIM render"), TEXT("CLAIM audio"),
	};
	for (int k = 0; k < 7; k++) {
		char name[64];
		snprintf(name, sizeof name, "exec_claim_%s", kClaimNames[k]);
		begin(name);
		g_s.claim = k;
		g_eng->Client = (decltype(g_eng->Client))fake(FK_CLIENT);
		g_eng->Render = (decltype(g_eng->Render))fake(FK_RENDER);
		g_eng->Audio = (decltype(g_eng->Audio))fake(FK_AUDIO);
		UBOOL r = g_eng->UEngine::Exec(kClaimCmd[k], *GLog);
		printf("result\t%s\t%d\n", name, r);
		dump_state();
	}
	begin("exec_null_optionals");
	GSys = NULL;
	GExec = NULL;
	{
		UBOOL r = g_eng->UEngine::Exec(TEXT("NOPE"), *GLog);
		printf("result\texec_null_optionals\t%d\n", r);
	}
	dump_state();
	begin("exec_flush");
	g_eng->Client = (decltype(g_eng->Client))fake(FK_CLIENT);
	{
		UBOOL r = g_eng->UEngine::Exec(TEXT("FLUSH"), *GLog);
		printf("result\texec_flush\t%d\n", r);
	}
	dump_state();
	begin("exec_crackurl_valid");
	{
		UBOOL r = g_eng->UEngine::Exec(TEXT("CRACKURL unreal://127.0.0.1:7777/Entry?Game=x?Listen#Portal"), *GLog);
		printf("result\texec_crackurl_valid\t%d\n", r);
	}
	dump_state();
	begin("exec_crackurl_bad");
	g_s.url_valid = 0;
	{
		UBOOL r = g_eng->UEngine::Exec(TEXT("CRACKURL bogus"), *GLog);
		printf("result\texec_crackurl_bad\t%d\n", r);
	}
	dump_state();
	begin("exec_unknown");
	{
		UBOOL r = g_eng->UEngine::Exec(TEXT("NOPE"), *GLog);
		printf("result\texec_unknown\t%d\n", r);
	}
	dump_state();

	// Key: console absent, console declines, console accepts.
	begin("key_no_console");
	setup_viewport(false, false, false, false);
	printf("result\tkey_no_console\t%d\n", g_eng->UEngine::Key(g_vp, (EInputKey)121));
	dump_state();
	begin("key_console_yes");
	setup_viewport(true, false, false, false);
	g_s.key_type = 1;
	printf("result\tkey_console_yes\t%d\n", g_eng->UEngine::Key(g_vp, (EInputKey)121));
	dump_state();
	begin("key_console_no");
	setup_viewport(true, false, false, false);
	g_s.key_type = 0;
	printf("result\tkey_console_no\t%d\n", g_eng->UEngine::Key(g_vp, (EInputKey)121));
	dump_state();

	// InputEvent: console, pre/process, and MainMenu yes/no translation.
	begin("ie_no_console");
	setup_viewport(false, false, false, false);
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_no_console\t%d\n", r);
	}
	dump_state();
	begin("ie_console_release");
	setup_viewport(true, false, false, false);
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)3, 0.125f);
		printf("result\tie_console_release\t%d\n", r);
	}
	dump_state();
	begin("ie_console_press");
	setup_viewport(true, false, false, false);
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_console_press\t%d\n", r);
	}
	dump_state();
	begin("ie_console_pre_fail");
	setup_viewport(true, false, false, false);
	g_s.pre = 0;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_console_pre_fail\t%d\n", r);
	}
	dump_state();
	begin("ie_console_process");
	setup_viewport(true, false, false, false);
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_console_process\t%d\n", r);
	}
	dump_state();
	begin("ie_console_process_fail");
	setup_viewport(true, false, false, false);
	g_s.proc = 0;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_console_process_fail\t%d\n", r);
	}
	dump_state();
	begin("ie_menu_yes");
	setup_viewport(true, true, true, true);
	set_chain({ TEXT("UnrealYesNoMenu") });
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_menu_yes\t%d\n", r);
	}
	dump_state();
	begin("ie_menu_no");
	setup_viewport(true, true, true, true);
	set_chain({ TEXT("Foo"), TEXT("UnrealQuitMenu") });
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)110, (EInputAction)1, 0.125f);
		printf("result\tie_menu_no\t%d\n", r);
	}
	dump_state();
	begin("ie_menu_other_class");
	setup_viewport(true, true, true, true);
	set_chain({ TEXT("Foo"), TEXT("Bar") });
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_menu_other_class\t%d\n", r);
	}
	dump_state();
	begin("ie_menu_nomatch_key");
	setup_viewport(true, true, true, true);
	set_chain({ TEXT("UnrealYesNoMenu") });
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)120, (EInputAction)1, 0.125f);
		printf("result\tie_menu_nomatch_key\t%d\n", r);
	}
	dump_state();
	begin("ie_hud_no_menu");
	setup_viewport(true, true, true, false);
	g_s.key_event = 1;
	{
		UBOOL r = g_eng->UEngine::InputEvent(g_vp, (EInputKey)121, (EInputAction)1, 0.125f);
		printf("result\tie_hud_no_menu\t%d\n", r);
	}
	dump_state();

	// Commandlet Main: editor sleep path with scripted clock, and the game path without language.
	begin("cmdlet_editor");
	GIsEditor = 1;
	g_s.config_lang = 1;
	printf("result\tcmdlet_editor\t%d\n", g_cmdlet->UServerCommandlet::Main(TEXT("")));
	dump_state();
	begin("cmdlet_game_no_lang");
	GIsEditor = 0;
	g_s.config_lang = 0;
	printf("result\tcmdlet_game_no_lang\t%d\n", g_cmdlet->UServerCommandlet::Main(TEXT("")));
	dump_state();
	return 0;
}
