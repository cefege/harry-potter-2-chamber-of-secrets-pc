// Original-object UConsole fixture harness. gen_console.sh rewrites the engine
// boundary of the pinned UnCon.cpp (service-equivalent hook calls, and the
// P_GET_*/P_FINISH marshalling of the exec natives replaced by harness
// globals) and links the original object against the hooks defined here.
// Every arithmetic statement of _Init/PreRender/PostRender/GetDrawWorld runs
// unmodified from the original object.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <cstdarg>
#define private public
#define protected public
#include "EnginePrivate.h"
#include "UnRender.h"
#undef private
#undef protected

UBOOL GIsEditor = 0;
// Native registration runs in static initializers of the original TU; no table is needed here.
BYTE GRegisterNative(INT, const Native&) { return 0; }
// Static initializers of the original TU allocate through GMalloc; back it with libc.
class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { return malloc( Count ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { return realloc( Original, Count ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

// Globals read by the rewritten exec natives (stand in for P_GET_STR/P_GET_INT).
const TCHAR* g_arg_str = NULL;
INT g_arg_int = 0;

static UConsole* g_console = NULL;
static UViewport* g_viewport = NULL;
static INT g_exec_ret = 0;
static UBOOL g_font_set = 1;
alignas(16) static char g_font_storage[64];

static const TCHAR kEngineText[] = L"Engine|Core";
static const TCHAR kCopyrightText[] = L"Copyright|Core";

// Hooks called from the rewritten UnCon.cpp.
void ConsoleHookInitExecution(UConsole*) { printf("init_exec\n"); }

const TCHAR* ConsoleHookLocalizeAnsi(const ANSICHAR* Key, const TCHAR* Package) {
    if (wcscmp(Package, L"Core") != 0) abort();
    if (strcmp(Key, "Engine") == 0) return kEngineText;
    if (strcmp(Key, "Copyright") == 0) return kCopyrightText;
    abort();
}

void ConsoleHookLog(UConsole*, const TCHAR* Text) { printf("log %ls\n", Text); }

void ConsoleHookEventMessage(UConsole*, const TCHAR* Data, EName Name) {
    printf("event name=%d data=%ls\n", (int)Name, Data);
}

void ConsoleHookTick(UConsole*, FLOAT DeltaTime) {
    uint32_t bits;
    memcpy(&bits, &DeltaTime, sizeof(bits));
    printf("tick %08x\n", bits);
}

INT ConsoleHookExec(UViewport* Viewport, const TCHAR* Cmd, FOutputDevice& Ar) {
    printf("exec vp_ok=%d ar_console=%d cmd=%ls ret=%d\n",
           Viewport == g_viewport ? 1 : 0,
           (void*)&Ar == (void*)static_cast<FOutputDevice*>(g_console) ? 1 : 0,
           Cmd, g_exec_ret);
    return g_exec_ret;
}

void ConsoleHookSave(const TCHAR* String, const TCHAR* Filename) {
    printf("save filename=%ls string=%ls\n", Filename, String);
}

UFont* ConsoleHookCreateFont(UViewport* Viewport, const TCHAR* Name, INT Height) {
    printf("font name=%ls height=%d -> %s\n", Name, Height, g_font_set ? "set" : "null");
    if (Viewport != g_viewport) abort();
    return g_font_set ? (UFont*)g_font_storage : NULL;
}

void ConsoleHookComputeRenderSize(FSceneNode* Frame) {
    printf("compute x=%d y=%d xb=%d yb=%d\n", Frame->X, Frame->Y, Frame->XB, Frame->YB);
}

static uint32_t bits_of(float f) {
    uint32_t b;
    memcpy(&b, &f, sizeof(b));
    return b;
}

static void run_exec(UConsole* c, const TCHAR* cmd, INT ret, void* frame_stack) {
    DWORD result = 0;
    g_arg_str = cmd;
    g_exec_ret = ret;
    c->UConsole::execConsoleCommand(*(FFrame*)frame_stack, (void*)&result);
    printf("exec_result=%u\n", (unsigned)result);
}

static void run_font(UConsole* c, const TCHAR* name, INT height, UBOOL set, void* frame_stack) {
    UFont* out = NULL;
    g_arg_str = name;
    g_arg_int = height;
    g_font_set = set;
    c->UConsole::execCreateNativeFont(*(FFrame*)frame_stack, (void*)&out);
    printf("font_result=%s\n", out ? "set" : "null");
}

alignas(16) static char g_con_storage[sizeof(UConsole)];
alignas(16) static char g_vp_storage[sizeof(UViewport)];
alignas(16) static char g_stack_storage[sizeof(FFrame)];

int main() {
    UConsole* c = (UConsole*)g_con_storage;
    UViewport* vp = (UViewport*)g_vp_storage;
    void* stack = g_stack_storage;
    g_console = c;
    g_viewport = vp;

    // _Init.
    c->UConsole::_Init(vp);
    printf("init top=%d border=%d vp=%d\n", c->TopLine, c->BorderSize, c->Viewport == vp ? 1 : 0);

    // Serialize.
    static const INT kNames[3] = {0, 1, 37};
    for (INT n : kNames) c->UConsole::Serialize(L"hello", (EName)n);

    // execConsoleCommand.
    run_exec(c, L"open map", 1, stack);
    run_exec(c, L"bogus", 0, stack);

    // execSaveTimeDemo.
    g_arg_str = L"fps dump";
    c->UConsole::execSaveTimeDemo(*(FFrame*)stack, NULL);

    // execCreateNativeFont.
    run_font(c, L"Arial", 12, 1, stack);
    run_font(c, L"Tiny", 6, 0, stack);

    // GetDrawWorld.
    for (UBOOL nd = 0; nd <= 1; ++nd) {
        c->bNoDrawWorld = nd;
        printf("draw_world nd=%d ret=%d\n", nd, c->UConsole::GetDrawWorld());
    }

    // PreRender / PostRender grid.
    static const INT kBorders[9] = {-3, 0, 1, 2, 3, 4, 5, 6, 7};
    static const FLOAT kPos[6] = {-0.5f, 0.0f, 0.25f, 0.5f, 1.0f, 1.5f};
    static const INT kFrames[7][4] = {
        {640, 480, 0, 0}, {800, 600, 10, 20}, {1024, 768, -4, 0}, {0, 0, 0, 0},
        {-1, -2, 3, 4}, {7, 9, 0, 0}, {4096, 2160, 1, 2},
    };
    static const FLOAT kTimes[4][2] = {{1.5f, 0.25f}, {0.125f, 0.125f}, {1000.3f, 999.7f}, {0.1f, 0.3f}};
    FSceneNode fr;
    for (INT bs : kBorders)
    for (FLOAT cp : kPos)
    for (const auto& f : kFrames)
    for (UBOOL nd = 0; nd <= 1; ++nd)
    for (const auto& t : kTimes) {
        c->BorderSize = bs;
        c->ConsolePos = cp;
        c->bNoDrawWorld = nd;
        vp->CurrentTime = FTime(t[0]);
        vp->LastUpdateTime = FTime(t[1]);
        fr.X = f[0];
        fr.Y = f[1];
        fr.XB = f[2];
        fr.YB = f[3];
        c->UConsole::PreRender(&fr);
        printf("pre bs=%d cp=%08x out=%d,%d,%d,%d cl=%d bl=%d bp=%d\n",
               bs, bits_of(cp), fr.X, fr.Y, fr.XB, fr.YB,
               c->ConsoleLines, c->BorderLines, c->BorderPixels);
        c->UConsole::PostRender(&fr);
        printf("post fx=%08x fy=%08x drew=%d frame=%d,%d,%d,%d\n",
               bits_of(c->FrameX), bits_of(c->FrameY), c->bDrewWorld ? 1 : 0,
               fr.X, fr.Y, fr.XB, fr.YB);
    }
    return 0;
}
