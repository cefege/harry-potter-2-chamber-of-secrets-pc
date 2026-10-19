// Oracle harness for the pawn_native_files slice.
// Compiled against the oracle UnPawn.cpp copy whose engine calls are rewritten to
// PawnFiles* hooks (PawnFilesHooks.h). The oracle natives otherwise run unmodified.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdarg>
#include <cwchar>
#include <cmath>
#include <string>
#include <utility>
#include <vector>

#define private public
#define protected public
#include "EnginePrivate.h"
#undef private
#undef protected
#include "PawnFilesHooks.h"

UBOOL GIsEditor = 0;
DWORD appCycles() { return 100; }
FGlobalMath GMath;
BYTE GRegisterNative(INT, const Native&) { return 0; }
// Engine name and log globals referenced by UnPawn.o. Real storage; the fixture never dispatches through them.
FName ENGINE_AlterDestination, ENGINE_EnemyNotVisible, ENGINE_HearNoise, ENGINE_LongFall, ENGINE_SeePlayer, ENGINE_SpecialHandling, ENGINE_Touch;
FOutputDevice* GLog = NULL;
FOutputDeviceError* GError = NULL;

// ---------------------------------------------------------------- engine globals
class HarnessMalloc : public FMalloc {
public:
    void* Malloc(DWORD Count, const TCHAR*) { return malloc(Count ? Count : 1); }
    void* Realloc(void* Original, DWORD Count, const TCHAR*) { return realloc(Original, Count ? Count : 1); }
    void Free(void* Original) { free(Original); }
    void DumpAllocs() {}
    void HeapCheck() {}
    void Init() {}
    void Exit() {}
};
static HarnessMalloc g_malloc;
FMalloc* GMalloc = &g_malloc;

alignas(16) static unsigned char g_sys_storage[sizeof(USystem)];
USystem* GSys = (USystem*)g_sys_storage;
BYTE* GPropAddr = NULL;
TCHAR GCdPath[256];

FURL::FURL(const TCHAR*) {}

// ---------------------------------------------------------------- object storage
alignas(16) static unsigned char g_pawn_storage[sizeof(APlayerPawn)];
alignas(16) static unsigned char g_level_storage[sizeof(ULevel)];
alignas(16) static unsigned char g_engine_storage[sizeof(UGameEngine)];
alignas(16) static unsigned char g_viewport_storage[sizeof(UViewport)];
alignas(16) static unsigned char g_input_storage[sizeof(UInput)];
alignas(16) static unsigned char g_player_storage[sizeof(UPlayer)];
alignas(16) static unsigned char g_actor_b_storage[sizeof(AActor)];
alignas(16) static unsigned char g_goal_storage[sizeof(AActor)];
alignas(16) static unsigned char g_class_storage[sizeof(UClass)];
alignas(16) static unsigned char g_texclass_storage[sizeof(UClass)];
alignas(16) static unsigned char g_prop_storage[sizeof(UProperty)];
alignas(16) static unsigned char g_entry_level_storage[sizeof(ALevelInfo)];
static ALevelInfo* const g_entry_level = (ALevelInfo*)g_entry_level_storage;
static std::wstring g_last_url_map;

static APlayerPawn* const g_pawn = (APlayerPawn*)g_pawn_storage;
static ULevel* const g_level = (ULevel*)g_level_storage;
static UGameEngine* const g_engine = (UGameEngine*)g_engine_storage;
static UViewport* const g_viewport = (UViewport*)g_viewport_storage;
static UInput* const g_input = (UInput*)g_input_storage;
static UPlayer* const g_player = (UPlayer*)g_player_storage;
static AActor* const g_actor_b = (AActor*)g_actor_b_storage;
static AActor* const g_goal = (AActor*)g_goal_storage;
static UClass* const g_class = (UClass*)g_class_storage;
static UClass* const g_texclass = (UClass*)g_texclass_storage;
static UProperty* const g_prop = (UProperty*)g_prop_storage;

// ---------------------------------------------------------------- frame / argument queue
static BYTE g_code[64];

BYTE* PawnFilesHarnessCode() { return g_code; }

// Mirrors appInit's CRC table fill (UnMisc.cpp) so TMap hashing matches production.
DWORD GCRCTable[256];
static const int g_crc_init = [] {
    for (DWORD iCRC = 0; iCRC < 256; iCRC++)
        for (DWORD c = iCRC << 24, j = 8; j != 0; j--)
            GCRCTable[iCRC] = c = c & 0x80000000 ? (c << 1) ^ 0x04c11db7 : (c << 1);
    return 0;
}();

void FFrame::Serialize(const TCHAR* V, EName Event) { (void)V; (void)Event; }

enum HKind { H_STR, H_INT, H_UBOOL, H_FLOAT, H_VEC, H_SKIP, H_RSTR, H_RFLOAT };

struct HArg {
    HKind kind;
    std::wstring s;
    INT i;
    DWORD u;
    FLOAT f;
    FVector v;
    void* slot;
};

static HArg h_make(HKind k) {
    HArg a;
    a.kind = k;
    a.i = 0;
    a.u = 0;
    a.f = 0.f;
    a.v.X = a.v.Y = a.v.Z = 0.f;
    a.slot = NULL;
    return a;
}
static HArg h_str(const wchar_t* s) { HArg a = h_make(H_STR); a.s = s; return a; }
static HArg h_int(INT i) { HArg a = h_make(H_INT); a.i = i; return a; }
static HArg h_bool(DWORD u) { HArg a = h_make(H_UBOOL); a.u = u; return a; }
static HArg h_vec(FLOAT x, FLOAT y, FLOAT z) { HArg a = h_make(H_VEC); a.v.X = x; a.v.Y = y; a.v.Z = z; return a; }
static HArg h_skip() { return h_make(H_SKIP); }
static HArg h_rstr(FString* slot) { HArg a = h_make(H_RSTR); a.slot = slot; return a; }
static HArg h_rflt(FLOAT* slot) { HArg a = h_make(H_RFLOAT); a.slot = slot; return a; }

static std::vector<HArg> g_args;
static size_t g_argpos = 0;

static void h_setargs(std::initializer_list<HArg> list) {
    g_args.assign(list.begin(), list.end());
    g_argpos = 0;
}

void PawnFilesStepArg(UObject* Context, void* const Result) {
    (void)Context;
    if (Result == NULL) return; // P_FINISH debugger probe: no argument.
    if (g_argpos >= g_args.size()) {
        fprintf(stderr, "harness: argument underflow at %zu\n", g_argpos);
        abort();
    }
    const HArg& a = g_args[g_argpos++];
    switch (a.kind) {
    case H_STR: *(FString*)Result = FString(a.s.c_str()); break;
    case H_INT: *(INT*)Result = a.i; break;
    case H_UBOOL: *(DWORD*)Result = a.u; break;
    case H_FLOAT: *(FLOAT*)Result = a.f; break;
    case H_VEC: *(FVector*)Result = a.v; break;
    case H_SKIP: break; // optional argument absent: default stays in place.
    case H_RSTR:
    case H_RFLOAT: GPropAddr = (BYTE*)a.slot; break;
    }
}

// ---------------------------------------------------------------- recording helpers
static std::vector<std::wstring> g_fx_list;

static void fx_add(const std::wstring& e) { g_fx_list.push_back(e); }

static std::wstring ws(const FString& f) {
    const TCHAR* p = *f;
    return p ? std::wstring(p) : std::wstring();
}

static std::string qs(const std::wstring& w) {
    std::string r = "\"";
    for (wchar_t c : w) {
        if (c == '\\') r += "\\\\";
        else if (c == '"') r += "\\\"";
        else if (c < 0x20 || c > 0x7e) {
            char b[16];
            snprintf(b, sizeof b, "\\u%04x", (unsigned)c);
            r += b;
        } else {
            r += (char)c;
        }
    }
    r += "\"";
    return r;
}

static std::string hf(FLOAT f) {
    uint32_t u;
    memcpy(&u, &f, sizeof u);
    char b[16];
    snprintf(b, sizeof b, "%08x", u);
    return b;
}

static std::wstring fx_join() {
    std::wstring out;
    for (size_t i = 0; i < g_fx_list.size(); ++i) {
        if (i) out += L";";
        out += g_fx_list[i];
    }
    return out;
}

static void rec(const char* entry, const char* cs, const std::string& body) {
    printf("entry=%s case=%s %s fx=%s\n", entry, cs, body.c_str(), qs(fx_join()).c_str());
    g_fx_list.clear();
}

// ---------------------------------------------------------------- world model
struct KV { std::wstring k; std::wstring v; };
struct FileEntry { std::wstring pattern; std::vector<std::wstring> names; };

static std::vector<KV> g_registry;
static std::vector<KV> g_skins;
static std::vector<KV> g_ini;
static bool g_cache_section = true;
static TMultiMap<FString, FString> g_cache_map;
static std::vector<KV> g_url_defaults;
static std::vector<FileEntry> g_files;
static bool g_has_gameengine = true;
static bool g_has_serverpkg = true;
static INT g_defaults_num = 1;
static std::wstring g_export_text;
static UBOOL g_move_ok = 1;
static bool g_has_player = true;
static bool g_has_viewport = true;
static bool g_has_input = true;
static UBOOL g_maintain = 0;
static FLOAT g_eff_fov = 90.f;
static FVector g_view_loc;
static FRotator g_view_rot;
static std::wstring g_exec_out;
static UBOOL g_exec_ret = 1;
static int g_best = 0; // 0 none, 1 actor_b, 2 goal
static FLOAT g_best_weight = 0.f;
static UBOOL g_probing = 0;

static FVector mkvec(FLOAT x, FLOAT y, FLOAT z) {
    FVector v;
    v.X = x;
    v.Y = y;
    v.Z = z;
    return v;
}

static void reset_world() {
    g_fx_list.clear();
    g_registry = {{L"Harry.A", L"Desc A"}, {L"Harry.B", L"Desc B"}, {L"Harry.C", L"Desc C"}};
    g_skins = {{L"HarryA", L"dA"}, {L"HarryB", L"dB"}, {L"Other", L"dO"}, {L"HarryC", L"dC"}};
    g_ini.clear();
    g_cache_section = true;
    g_url_defaults = {{L"Name", L"Val"}, {L"Other", L"X"}};
    g_files.clear();
    GCdPath[0] = 0;
    GSys->Paths.Empty();
    g_has_gameengine = true;
    g_has_serverpkg = true;
    g_defaults_num = 1;
    g_export_text = L"HARRYSKINS,OTHER";
    g_move_ok = 1;
    g_has_player = true;
    g_has_viewport = true;
    g_has_input = true;
    g_maintain = 0;
    g_eff_fov = 90.f;
    g_view_loc = mkvec(0.f, 0.f, 0.f);
    g_view_rot.Pitch = 0;
    g_view_rot.Yaw = 0;
    g_view_rot.Roll = 0;
    g_exec_out.clear();
    g_exec_ret = 1;
    g_best = 0;
    g_best_weight = 0.f;
    g_probing = 0;
    g_pawn->SpecialPause = 7.5f;
    g_pawn->bShootSpecial = 1;
    g_pawn->SpecialGoal = NULL;
}

// Mirrors the world's object graph into the oracle's pointers.
static void apply_world() {
    g_level->Engine = g_engine;
    g_pawn->Player = g_has_player ? g_player : NULL;
    g_viewport->Input = g_has_input ? g_input : NULL;
}

static void set_paths(std::vector<std::wstring> paths) {
    GSys->Paths.Empty();
    for (const auto& p : paths) GSys->Paths.AddItem(FString(p.c_str()));
}

static void add_file(const wchar_t* pattern, std::vector<std::wstring> names) {
    FileEntry e;
    e.pattern = pattern;
    e.names = std::move(names);
    g_files.push_back(std::move(e));
}

// ---------------------------------------------------------------- hooks
ULevel* PawnFilesGetLevel(const void*) { return g_level; }
FString PawnFilesLastURLMap() {
    fx_add(L"lasturl");
    return FString(g_last_url_map.c_str());
}
void PawnFilesEntryLevelResult(APlayerPawn* self, void* const result) {
    (void)self;
    fx_add(L"entrylevel");
    *(ALevelInfo**)result = g_entry_level;
}

UClass* PawnFilesFindObject(const TCHAR* name) {
    fx_add(std::wstring(L"findclass:") + name);
    return g_has_gameengine ? g_class : NULL;
}

UClass* PawnFilesFindObjectChecked(const TCHAR* name) {
    fx_add(std::wstring(L"findclass:") + name);
    return g_class;
}

UClass* PawnFilesTextureClass() { return g_texclass; }

void PawnFilesRegistryObjects(TArray<FRegistryObjectInfo>& list, UClass* meta) {
    const bool tex = (meta == g_texclass);
    fx_add(tex ? L"registry:tex" : L"registry:class");
    const std::vector<KV>& src = tex ? g_skins : g_registry;
    list.Empty();
    for (const auto& kv : src) {
        FRegistryObjectInfo info;
        info.Object = FString(kv.k.c_str());
        info.Description = FString(kv.v.c_str());
        list.AddItem(info);
    }
}

UProperty* PawnFilesFindProperty(UClass* cls, const TCHAR* name) {
    (void)cls;
    fx_add(std::wstring(L"findfield:") + name);
    return g_has_serverpkg ? g_prop : NULL;
}

INT PawnFilesDefaultsNum(UClass* cls) {
    (void)cls;
    return g_defaults_num;
}

void PawnFilesExportText(UProperty* prop, TCHAR* out) {
    (void)prop;
    fx_add(L"exporttext");
    wcscpy(out, g_export_text.c_str());
}

TMultiMap<FString, FString>* PawnFilesCacheSection(const TCHAR* section, UBOOL force, UBOOL cnst, const TCHAR* filename) {
    (void)force;
    (void)cnst;
    (void)filename;
    fx_add(std::wstring(L"cachesec:") + section);
    if (!g_cache_section) return NULL;
    g_cache_map.Empty();
    for (const auto& kv : g_ini) g_cache_map.Add(kv.k.c_str(), kv.v.c_str());
    return &g_cache_map;
}

UBOOL PawnFilesCacheGetString(const TCHAR* section, const TCHAR* key, TCHAR* value, INT size, const TCHAR* filename) {
    (void)section;
    (void)filename;
    fx_add(std::wstring(L"cacheget:") + key);
    for (const auto& kv : g_ini) {
        if (kv.k == key) {
            wcsncpy(value, kv.v.c_str(), size - 1);
            value[size - 1] = 0;
            return 1;
        }
    }
    value[0] = 0;
    return 0;
}

void PawnFilesCacheSetString(const TCHAR* section, const TCHAR* key, const TCHAR* value, const TCHAR* filename) {
    (void)section;
    (void)filename;
    fx_add(std::wstring(L"cacheset:") + key + L"=" + value);
    for (auto& kv : g_ini) {
        if (kv.k == key) {
            kv.v = value;
            return;
        }
    }
    g_ini.push_back({key, value});
}

void PawnFilesDebugf(const TCHAR* fmt, ...) {
    std::wstring out;
    va_list ap;
    va_start(ap, fmt);
    for (const TCHAR* p = fmt; *p; ++p) {
        if (p[0] == '%' && p[1] == 's') {
            const TCHAR* a = va_arg(ap, const TCHAR*);
            out += a ? a : L"(null)";
            ++p;
        } else {
            out += *p;
        }
    }
    va_end(ap);
    fx_add(L"log:" + out);
}

UBOOL PawnFilesMove(const TCHAR* dest, const TCHAR* src) {
    fx_add(std::wstring(L"move:") + src + L"->" + dest + (g_move_ok ? L"=1" : L"=0"));
    return g_move_ok;
}

TArray<FString> PawnFilesFindFiles(const TCHAR* pattern, UBOOL files, UBOOL dirs) {
    (void)files;
    (void)dirs;
    fx_add(std::wstring(L"find:") + pattern);
    TArray<FString> out;
    for (const auto& e : g_files) {
        if (e.pattern == pattern) {
            for (const auto& n : e.names) out.AddItem(FString(n.c_str()));
            break;
        }
    }
    return out;
}

FString PawnFilesMapExt() { return FString(L"unr"); }

void PawnFilesURLAddOption(FURL& url, const TCHAR* option) {
    (void)url;
    fx_add(std::wstring(L"addopt:") + option);
}

void PawnFilesURLSaveConfig(const FURL& url, const TCHAR* section, const TCHAR* item, const TCHAR* filename) {
    (void)url;
    fx_add(std::wstring(L"save:") + section + L"/" + item + L"/" + filename);
}

void PawnFilesURLLoadConfig(FURL& url, const TCHAR* section, const TCHAR* filename) {
    (void)url;
    fx_add(std::wstring(L"loadcfg:") + section + L"/" + filename);
}

const TCHAR* PawnFilesURLGetOption(const FURL& url, const TCHAR* match, const TCHAR* def) {
    (void)url;
    fx_add(std::wstring(L"getopt:") + match);
    for (const auto& kv : g_url_defaults) {
        if (std::wstring(match) == kv.k + L"=") return kv.v.c_str();
    }
    return def;
}

UViewport* PawnFilesCastViewport(UObject* player) {
    (void)player;
    fx_add(L"castviewport");
    return g_has_viewport ? g_viewport : NULL;
}

void PawnFilesResetConfig(UClass* cls) {
    (void)cls;
    fx_add(L"reset");
}

FLOAT PawnFilesFindPathTowardBest(APawn* self, AActor*& best, INT clear, FLOAT min_weight, UBOOL predict) {
    (void)self;
    (void)min_weight;
    fx_add(std::wstring(L"findpath:clear=") + (clear ? L"1" : L"0") + (predict ? L",predict=1" : L",predict=0"));
    if (g_best == 1) best = g_actor_b;
    else if (g_best == 2) best = g_goal;
    else best = NULL;
    return g_best_weight;
}

UBOOL PawnFilesIsProbing(UObject* actor, FName name) {
    (void)actor;
    (void)name;
    fx_add(L"probing");
    return g_probing;
}

void PawnFilesHandleSpecial(APawn* self, AActor*& best) {
    (void)self;
    (void)best;
    fx_add(L"handle");
}

UViewport* PawnFilesScreenViewport() { return g_viewport; }

UBOOL PawnFilesMaintainVertical() { return g_maintain; }

FLOAT PawnFilesEffectiveFov(FLOAT authored, INT x, INT y, UBOOL maintain) {
    const std::string auth = hf(authored);
    fx_add(L"eff:auth=" + std::wstring(auth.begin(), auth.end()) +
           L",x=" + std::to_wstring(x) + L",y=" + std::to_wstring(y) + L",m=" + std::to_wstring(maintain));
    return g_eff_fov;
}

void PawnFilesCalcView(AActor*& view, FVector& loc, FRotator& rot) {
    (void)view;
    fx_add(L"calcview");
    loc = g_view_loc;
    rot = g_view_rot;
}

static void write_output(FOutputDevice& ar, const std::wstring& text) {
    static_cast<FString&>(static_cast<FStringOutputDevice&>(ar)) = FString(text.c_str());
}

UBOOL PawnFilesPlayerExec(UObject* player, const TCHAR* cmd, FOutputDevice& ar) {
    (void)player;
    fx_add(std::wstring(L"exec_player:") + cmd);
    write_output(ar, g_exec_out);
    return g_exec_ret;
}

UBOOL PawnFilesEngineExec(UObject* engine, const TCHAR* cmd, FOutputDevice& ar) {
    (void)engine;
    fx_add(std::wstring(L"exec_engine:") + cmd);
    write_output(ar, g_exec_out);
    return g_exec_ret;
}

// ---------------------------------------------------------------- native runners
static void run_get_next_int(const char* cs, const wchar_t* cls, INT idx) {
    h_setargs({h_str(cls), h_int(idx)});
    FFrame stack((UObject*)g_pawn);
    FString res;
    static_cast<AActor*>(g_pawn)->execGetNextInt(stack, &res);
    rec("get_next_int", cs, "result=" + qs(ws(res)));
}

static void run_get_next_int_desc(const char* cs, const wchar_t* cls, INT idx) {
    FString entry(L"seedE");
    FString desc(L"seedD");
    h_setargs({h_str(cls), h_int(idx), h_rstr(&entry), h_rstr(&desc)});
    FFrame stack((UObject*)g_pawn);
    DWORD dummy = 0;
    static_cast<AActor*>(g_pawn)->execGetNextIntDesc(stack, &dummy);
    rec("get_next_int_desc", cs, "entry=" + qs(ws(entry)) + " desc=" + qs(ws(desc)));
}

static void run_get_cache_entry(const char* cs, INT num) {
    FString guid(L"seedG");
    FString fname(L"seedF");
    h_setargs({h_int(num), h_rstr(&guid), h_rstr(&fname)});
    FFrame stack((UObject*)g_pawn);
    DWORD res = 0;
    static_cast<AActor*>(g_pawn)->execGetCacheEntry(stack, &res);
    std::wstring order;
    if (g_cache_section) {
        for (TMultiMap<FString, FString>::TIterator It(g_cache_map); It; ++It) {
            order += std::wstring(*It.Key()) + L"=" + std::wstring(*It.Value()) + L";";
        }
    }
    rec("get_cache_entry", cs,
        "result=" + std::to_string(res) + " guid=" + qs(ws(guid)) + " filename=" + qs(ws(fname)) +
            " order=" + qs(order));
}

static void run_move_cache_entry(const char* cs, const wchar_t* guid, bool skip, const wchar_t* newname) {
    h_setargs({h_str(guid), skip ? h_skip() : h_str(newname)});
    FFrame stack((UObject*)g_pawn);
    DWORD res = 0;
    static_cast<AActor*>(g_pawn)->execMoveCacheEntry(stack, &res);
    rec("move_cache_entry", cs, "result=" + std::to_string(res));
}

static void run_check_valid(const char* cs, const wchar_t* pkg, const wchar_t* mesh) {
    h_setargs({h_str(pkg), h_str(mesh)});
    FFrame stack((UObject*)g_pawn);
    DWORD res = 0;
    static_cast<APawn*>(g_pawn)->execCheckValidSkinPackage(stack, &res);
    rec("check_valid_skin_package", cs, "result=" + std::to_string(res));
}

static void run_update_url(const char* cs, const wchar_t* option, const wchar_t* value, DWORD save) {
    h_setargs({h_str(option), h_str(value), h_bool(save)});
    FFrame stack((UObject*)g_pawn);
    DWORD dummy = 0;
    g_pawn->execUpdateURL(stack, &dummy);
    rec("update_url", cs, "done=1");
}

static void run_get_default_url(const char* cs, const wchar_t* option) {
    h_setargs({h_str(option)});
    FFrame stack((UObject*)g_pawn);
    FString res;
    g_pawn->execGetDefaultURL(stack, &res);
    rec("get_default_url", cs, "result=" + qs(ws(res)));
}

static void run_get_next_skin(const char* cs, const wchar_t* prefix, const wchar_t* current, INT dir) {
    FString name(L"seedN");
    FString desc(L"seedD");
    h_setargs({h_str(prefix), h_str(current), h_int(dir), h_rstr(&name), h_rstr(&desc)});
    FFrame stack((UObject*)g_pawn);
    DWORD dummy = 0;
    static_cast<AActor*>(g_pawn)->execGetNextSkin(stack, &dummy);
    rec("get_next_skin", cs, "name=" + qs(ws(name)) + " desc=" + qs(ws(desc)));
}

static void run_reset_keyboard(const char* cs) {
    h_setargs({});
    FFrame stack((UObject*)g_pawn);
    DWORD dummy = 0;
    g_pawn->execResetKeyboard(stack, &dummy);
    rec("reset_keyboard", cs, "done=1");
}

static void run_find_best(const char* cs, FLOAT weight, UBOOL predict) {
    FLOAT w = weight;
    h_setargs({h_rflt(&w), h_bool(predict)});
    FFrame stack((UObject*)g_pawn);
    AActor* res = NULL;
    static_cast<APawn*>(g_pawn)->execFindBestInventoryPath(stack, &res);
    const char* tag = res == NULL ? "null" : (res == g_actor_b ? "b" : (res == g_goal ? "goal" : "other"));
    const char* goal = g_pawn->SpecialGoal == NULL ? "null" : (g_pawn->SpecialGoal == g_actor_b ? "b" : (g_pawn->SpecialGoal == g_goal ? "goal" : "other"));
    rec("find_best_inventory_path", cs,
        "weight=" + hf(w) + " result=" + tag + " pause=" + hf(g_pawn->SpecialPause) +
            " shoot=" + std::to_string((int)g_pawn->bShootSpecial) + " goal=" + goal);
}

static void run_get_map_name(const char* cs, const wchar_t* prefix, const wchar_t* map, INT dir) {
    h_setargs({h_str(prefix), h_str(map), h_int(dir)});
    FFrame stack((UObject*)g_pawn);
    FString res;
    static_cast<AActor*>(g_pawn)->execGetMapName(stack, &res);
    rec("get_map_name", cs, "result=" + qs(ws(res)));
}

static void run_console_command(const char* cs, const wchar_t* cmd) {
    h_setargs({h_str(cmd)});
    FFrame stack((UObject*)g_pawn);
    FString res;
    g_pawn->execConsoleCommand(stack, &res);
    rec("console_command", cs, "result=" + qs(ws(res)));
}

static void run_screen_to_world(const char* cs, FLOAT sx, FLOAT sy, FLOAT sz) {
    h_setargs({h_vec(sx, sy, sz)});
    FFrame stack((UObject*)g_pawn);
    FVector res = mkvec(0.f, 0.f, 0.f);
    g_pawn->execScreenToWorld(stack, &res);
    rec("screen_to_world", cs, "result=(" + hf(res.X) + "," + hf(res.Y) + "," + hf(res.Z) + ")");
}

static void run_get_url_map(const char* cs, const wchar_t* map) {
    g_last_url_map = map;
    FFrame stack((UObject*)g_pawn);
    FString res;
    static_cast<AActor*>(g_pawn)->execGetURLMap(stack, &res);
    rec("get_url_map", cs, "result=" + qs(ws(res)));
}

static void run_entry_level(const char* cs) {
    FFrame stack((UObject*)g_pawn);
    ALevelInfo* res = NULL;
    g_pawn->execGetEntryLevel(stack, &res);
    rec("get_entry_level", cs, std::string("result=") + (res == g_entry_level ? "entry" : "other"));
}

// ---------------------------------------------------------------- cases
static void default_ini() {
    g_ini = {{L"g1", L"a.unr"}, {L"g2", L""}, {L"g3", L"c.unr"}};
}

static void case_get_next_int() {
    reset_world(); apply_world(); run_get_next_int("i0", L"Harry", 0);
    reset_world(); apply_world(); run_get_next_int("i2", L"Harry", 2);
    reset_world(); apply_world(); run_get_next_int("i3", L"Harry", 3);
    reset_world(); g_registry.clear(); apply_world(); run_get_next_int("i_empty", L"Harry", 0);
}

static void case_get_next_int_desc() {
    reset_world(); apply_world(); run_get_next_int_desc("d1", L"Harry", 1);
    reset_world(); apply_world(); run_get_next_int_desc("d3", L"Harry", 3);
    reset_world(); g_registry.clear(); apply_world(); run_get_next_int_desc("d_empty", L"Harry", 0);
}

static void case_get_cache_entry() {
    reset_world(); default_ini(); apply_world(); run_get_cache_entry("n0", 0);
    reset_world(); default_ini(); apply_world(); run_get_cache_entry("n1", 1);
    reset_world(); default_ini(); apply_world(); run_get_cache_entry("n2", 2);
    reset_world(); default_ini(); g_cache_section = false; apply_world(); run_get_cache_entry("n_nosec", 0);
}

static void case_move_cache_entry() {
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr", L"System\\Engine.u"}); apply_world();
    run_move_cache_entry("m1", L"g1", true, L"");
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr", L"System\\Engine.u"}); apply_world();
    run_move_cache_entry("m2", L"g1", false, L"Dir/New.unr");
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr"}); apply_world();
    run_move_cache_entry("m3", L"g1", false, L"Nodot");
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr"}); g_move_ok = 0; apply_world();
    run_move_cache_entry("m4", L"g1", true, L"");
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr"}); apply_world();
    run_move_cache_entry("m5", L"zz", true, L"");
    reset_world(); g_ini = {{L"g3", L""}}; set_paths({L"Maps\\Start.unr"}); apply_world();
    run_move_cache_entry("m6", L"g3", true, L"");
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr"}); apply_world();
    run_move_cache_entry("m7", L"g1", false, L"x.bin");
    reset_world(); g_ini = {{L"g1", L"Orig.unr"}}; set_paths({L"Maps\\Start.unr"}); apply_world();
    run_move_cache_entry("m8", L"g1", false, L"C:\\Dir\\q.unr");
}

static void case_check_valid() {
    reset_world(); g_has_gameengine = false; apply_world(); run_check_valid("c1", L"HARRYSKINS", L"HARRY");
    reset_world(); g_has_serverpkg = false; apply_world(); run_check_valid("c2", L"HARRYSKINS", L"HARRY");
    reset_world(); g_defaults_num = 0; apply_world(); run_check_valid("c3", L"HARRYSKINS", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c4", L"UNREALSHARE", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c5", L"BOTPACK", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c6", L"HARRYSKINS", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c7", L"BOSSSKINS", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c8", L"TCOWMESHSKINS", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c9", L"FOO", L"HARRY");
    reset_world(); g_export_text = L"HARRYSKINS"; apply_world(); run_check_valid("c10", L"HARRYSKINS", L"HARRY");
    reset_world(); apply_world(); run_check_valid("c11", L"harryskins", L"harry");
}

static void case_update_url() {
    reset_world(); apply_world(); run_update_url("u0", L"Name", L"Val", 0);
    reset_world(); apply_world(); run_update_url("u1", L"Name", L"Val", 1);
}

static void case_get_default_url() {
    reset_world(); apply_world(); run_get_default_url("found", L"Name");
    reset_world(); apply_world(); run_get_default_url("missing", L"Missing");
    reset_world(); apply_world(); run_get_default_url("prefix", L"Nam");
}

static void case_get_next_skin() {
    reset_world(); apply_world(); run_get_next_skin("s_next", L"Harry", L"HarryA", 1);
    reset_world(); apply_world(); run_get_next_skin("s_prev", L"Harry", L"HarryB", -1);
    reset_world(); apply_world(); run_get_next_skin("s_cur", L"Harry", L"HarryB", 0);
    reset_world(); apply_world(); run_get_next_skin("s_wrap", L"Harry", L"HarryC", 1);
    reset_world(); apply_world(); run_get_next_skin("s_nocur", L"Harry", L"Nope", 1);
    reset_world(); apply_world(); run_get_next_skin("s_noprefix", L"Zzz", L"HarryA", 1);
    reset_world(); g_skins.clear(); apply_world(); run_get_next_skin("s_empty", L"Harry", L"HarryA", 1);
}

static void case_reset_keyboard() {
    reset_world(); apply_world(); run_reset_keyboard("r_input");
    reset_world(); g_has_input = false; apply_world(); run_reset_keyboard("r_noinput");
    reset_world(); g_has_viewport = false; apply_world(); run_reset_keyboard("r_noviewport");
}

static void case_find_best() {
    reset_world(); g_best = 1; g_best_weight = 2.f; g_pawn->SpecialGoal = g_goal; apply_world();
    run_find_best("f_improved", 1.f, 0);
    reset_world(); g_best = 2; g_best_weight = 2.f; g_pawn->SpecialGoal = g_goal; apply_world();
    run_find_best("f_goal", 1.f, 0);
    reset_world(); g_best = 1; g_best_weight = 0.5f; apply_world();
    run_find_best("f_notimproved", 1.f, 0);
    reset_world(); g_best = 1; g_best_weight = 2.f; g_probing = 1; apply_world();
    run_find_best("f_handle", 1.f, 0);
    reset_world(); g_best = 1; g_best_weight = 2.f; apply_world();
    run_find_best("f_predict", 1.f, 1);
    reset_world(); g_best = 0; g_best_weight = 0.f; apply_world();
    run_find_best("f_none", 0.f, 0);
}

static void case_get_map_name() {
    reset_world(); set_paths({L"Maps\\*.unr"});
    add_file(L"Maps\\*.unr", {L"Maps\\Alpha.unr", L"Maps\\Beta.unr"}); apply_world();
    run_get_map_name("g_basic", L"", L"Alpha.unr", 0);

    reset_world(); set_paths({L"Maps\\*.unr"});
    add_file(L"Maps\\*.unr", {L"Maps\\Alpha.unr", L"Maps\\Beta.unr"}); apply_world();
    run_get_map_name("g_dir_plus", L"", L"Beta.unr", 1);

    reset_world(); set_paths({L"Maps\\*.unr"});
    add_file(L"Maps\\*.unr", {L"Maps\\Alpha.unr", L"Maps\\Beta.unr"}); apply_world();
    run_get_map_name("g_dir_minus", L"", L"Alpha.unr", -1);

    reset_world(); set_paths({L"Maps\\*.unr"});
    add_file(L"Maps\\*.unr", {L"Maps\\Alpha.unr", L"Maps\\Beta.unr"}); apply_world();
    run_get_map_name("g_nomatch", L"", L"Zeta.unr", 0);

    reset_world(); set_paths({L"Maps\\*.unr"}); GCdPath[0] = 0;
    wcscpy(GCdPath, L"C:\\Hp2\\");
    add_file(L"Maps\\*.unr", {L"Maps\\Alpha.unr"});
    add_file((std::wstring(GCdPath) + L"System" + std::wstring(PATH_SEPARATOR) + L"Maps\\*.unr").c_str(), {L"Maps\\Gamma.unr"});
    apply_world();
    run_get_map_name("g_cd", L"", L"Gamma.unr", 0);

    reset_world(); set_paths({L"Maps\\*.unr", L"Maps\\*.unr"});
    add_file(L"Maps\\*.unr", {L"Maps\\Alpha.unr", L"Maps\\Beta.unr"}); apply_world();
    run_get_map_name("g_dedupe", L"", L"Beta.unr", 0);

    reset_world(); apply_world();
    run_get_map_name("g_empty", L"", L"x.unr", 0);

    reset_world(); set_paths({L"Maps\\*.unr"});
    add_file(L"Maps\\Al*.unr", {L"Maps\\Alpha.unr"}); apply_world();
    run_get_map_name("g_prefix", L"Al", L"Alpha.unr", 0);

    reset_world(); set_paths({L"Maps\\*.txt"}); apply_world();
    run_get_map_name("g_nonunr", L"", L"Alpha.unr", 0);

    reset_world(); set_paths({L"Maps\\*.unr"});
    add_file(L"Maps\\*.unr", {L"X\\Y\\Alpha.unr"}); apply_world();
    run_get_map_name("g_stripdir", L"", L"Alpha.unr", 0);
}

static void case_console_command() {
    reset_world(); g_exec_out = L"Result text"; apply_world();
    run_console_command("cc_player", L"Test Cmd");
    reset_world(); g_has_player = false; g_exec_out = L"Engine out"; apply_world();
    run_console_command("cc_engine", L"Test Cmd");
    reset_world(); g_exec_out = L""; apply_world();
    run_console_command("cc_empty", L"Test Cmd");
}

static void case_screen_to_world() {
    reset_world();
    g_viewport->SizeX = 640; g_viewport->SizeY = 480;
    g_eff_fov = 90.f; g_maintain = 0;
    g_view_loc = mkvec(0.f, 0.f, 0.f); g_view_rot.Pitch = 0; g_view_rot.Yaw = 0; g_view_rot.Roll = 0;
    g_pawn->FovAngle = 90.f; g_pawn->Location = mkvec(0.f, 0.f, 0.f);
    apply_world(); run_screen_to_world("sw1", 100.f, 50.f, 2.f);

    reset_world();
    g_viewport->SizeX = 800; g_viewport->SizeY = 600;
    g_eff_fov = 60.f; g_maintain = 0;
    g_view_loc = mkvec(10.f, 20.f, 30.f); g_view_rot.Pitch = 1000; g_view_rot.Yaw = 2000; g_view_rot.Roll = 3000;
    g_pawn->FovAngle = 60.f; g_pawn->Location = mkvec(10.f, 20.f, 30.f);
    apply_world(); run_screen_to_world("sw2", 0.f, 0.f, 1.f);

    reset_world();
    g_viewport->SizeX = 640; g_viewport->SizeY = 480;
    g_eff_fov = 90.f; g_maintain = 0;
    g_view_loc = mkvec(0.f, 0.f, 0.f); g_view_rot.Pitch = 0; g_view_rot.Yaw = 0; g_view_rot.Roll = 0;
    g_pawn->FovAngle = 90.f; g_pawn->Location = mkvec(0.f, 0.f, 0.f);
    apply_world(); run_screen_to_world("sw3", 320.f, 240.f, 0.5f);

    reset_world();
    g_viewport->SizeX = 1024; g_viewport->SizeY = 768;
    g_eff_fov = 75.f; g_maintain = 1;
    g_view_loc = mkvec(0.f, 0.f, 0.f); g_view_rot.Pitch = 0; g_view_rot.Yaw = 0; g_view_rot.Roll = 0;
    g_pawn->FovAngle = 75.f; g_pawn->Location = mkvec(0.f, 0.f, 0.f);
    apply_world(); run_screen_to_world("sw4", -50.5f, 700.f, 10.f);
}

static void case_get_url_map() {
    reset_world(); apply_world(); run_get_url_map("u_map", L"Start");
    reset_world(); apply_world(); run_get_url_map("u_path", L"Maps\\Start.unr");
    reset_world(); apply_world(); run_get_url_map("u_empty", L"");
}

static void case_get_entry_level() {
    reset_world(); apply_world(); run_entry_level("e_level");
}

int main() {
    GSys->CachePath = FString(L"Cache");
    GSys->CacheExt = FString(L".tmp");
    wcscpy(GCdPath, L"");

    case_get_next_int();
    case_get_next_int_desc();
    case_get_cache_entry();
    case_move_cache_entry();
    case_check_valid();
    case_update_url();
    case_get_default_url();
    case_get_next_skin();
    case_reset_keyboard();
    case_find_best();
    case_get_map_name();
    case_console_command();
    case_screen_to_world();
    case_get_url_map();
    case_get_entry_level();
    return 0;
}
