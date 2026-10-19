// Original-object modern third-person input fixture harness. gen_pawn_native.sh
// rewrites the reflection and state calls of the pinned UnPawn.cpp to the
// PawnInput* hooks defined here. The Begin/Finish/Apply bodies and their float
// math run unmodified from the original object.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdarg>
#include <cwchar>
#include <cmath>
#define private public
#define protected public
#include "EnginePrivate.h"
#undef private
#undef protected

UBOOL GIsEditor = 0;
DWORD appCycles() { return 100; }
DOUBLE appSqrt(DOUBLE x) { return sqrt(x); }
DOUBLE appSin(DOUBLE x) { return sin(x); }
DOUBLE appAtan2(DOUBLE y, DOUBLE x) { return atan2(y, x); }
DOUBLE appPow(DOUBLE x, DOUBLE y) { return pow(x, y); }
UBOOL appIsNan(DOUBLE x) { return isnan(x); }
FGlobalMath GMath;
BYTE GRegisterNative(INT, const Native&) { return 0; }

enum { kBoolCount = 13, kStateNodes = 3, kBoolStride = 16 };
static const wchar_t* const kBoolNames[kBoolCount] = {
    L"bScreenRelativeMovement", L"bLockedOnTarget", L"bFixedFaceDirection",
    L"bInDuelingMode", L"bReverseInput", L"bKeepStationary", L"bLockOutForward",
    L"bLockOutBackward", L"bLockOutStrafeLeft", L"bLockOutStrafeRight", L"bIsAiming",
    L"bE3DemoLockout", L"bAutoCenterCamera",
};
static const uint32_t kBitsVariants[8] = {
    0u, 1u << 5, 1u << 1, 1u << 10, (1u << 6) | (1u << 8), (1u << 9) | (1u << 7),
    1u << 12, (1u << 2) | (1u << 4),
};

// Harry class candidates: g_harry and g_harry2 both carry the path HGame.Harry.
alignas(16) static char g_class_storage[4][sizeof(UClass)];
static UClass* const g_harry = (UClass*)g_class_storage[0];
static UClass* const g_harry2 = (UClass*)g_class_storage[1];
static UClass* const g_base = (UClass*)g_class_storage[2];
static UClass* const g_other = (UClass*)g_class_storage[3];

alignas(16) static char g_prop_storage[kBoolCount + 1][512];
static_assert(sizeof(UBoolProperty) <= 512 && sizeof(UObjectProperty) <= 512, "prop storage");
alignas(16) static char g_pawn_storage[sizeof(APlayerPawn) + 512];
static const INT kBoolBase = (INT)sizeof(APlayerPawn);
alignas(16) static char g_view_storage[sizeof(AActor)];
alignas(16) static char g_state_storage[kStateNodes][sizeof(UState)];
static UState* g_state(int i) { return (UState*)g_state_storage[i]; }

static UClass* g_obj_class;
static UClass* g_base_super;
static uint32_t g_missing_mask;
static UBOOL g_walk_present, g_cam_present, g_boss_set;
static int g_logs;
static const wchar_t* g_last_log;

static int bool_index(const TCHAR* name) {
    for (int i = 0; i < kBoolCount; ++i)
        if (wcscmp(name, kBoolNames[i]) == 0) return i;
    abort();
}

// Hooks called from the rewritten UnPawn.cpp.
UBOOL PawnInputIsA(const APlayerPawn*, const UClass* klass) { return klass == g_obj_class; }
UClass* PawnInputGetClass(APlayerPawn*) { return g_obj_class; }
UClass* PawnInputClassSuper(UClass* klass) { return klass == g_base ? g_base_super : NULL; }
UBOOL PawnInputClassPathEquals(UClass* klass, const TCHAR* path) {
    return (klass == g_harry || klass == g_harry2) && wcscmp(path, L"HGame.Harry") == 0;
}
UBoolProperty* PawnInputFindBool(UClass* klass, const TCHAR* name) {
    if (klass != g_harry && klass != g_harry2) return NULL;
    const int i = bool_index(name);
    if (klass == g_harry2 && ((g_missing_mask >> i) & 1u)) return NULL;
    return (UBoolProperty*)g_prop_storage[i];
}
UObjectProperty* PawnInputFindObject(UClass* klass, const TCHAR* name) {
    if (klass != g_harry && klass != g_harry2) return NULL;
    if (wcscmp(name, L"BossTarget") != 0) abort();
    return (UObjectProperty*)g_prop_storage[kBoolCount];
}
UState* PawnInputStateNode(UObject* obj) {
    if ((void*)obj == (void*)g_pawn_storage) return g_walk_present ? g_state(0) : NULL;
    if ((void*)obj == (void*)g_view_storage) return g_cam_present ? g_state(2) : NULL;
    return NULL;
}
UState* PawnInputStateSuper(UState* state) { return state == g_state(0) ? g_state(1) : NULL; }
UBOOL PawnInputStateNameEquals(UState* state, const TCHAR* name) {
    if (state == g_state(0)) return wcscmp(name, L"PlayerWalking") == 0;
    if (state == g_state(1)) return wcscmp(name, L"PlayerPawnBase") == 0;
    if (state == g_state(2)) return wcscmp(name, L"StateStandardCam") == 0;
    return 0;
}

// The rewritten UnPawn.cpp routes debugf to this hook (see gen_pawn_native.sh).
void PawnInputDebugf(EName, const TCHAR*, ...) {
    va_list ap;
    va_start(ap, nullptr);
    g_logs++;
    g_last_log = va_arg(ap, const TCHAR*);
    va_end(ap);
}
// Non-input debugf sites are never reached by these entries; abort if one is.
void PawnInputDebugf(const TCHAR*, ...) { abort(); }

static uint32_t fbits(float f) {
    uint32_t u;
    memcpy(&u, &f, sizeof u);
    return u;
}

static uint32_t* harry_word(int i) { return (uint32_t*)(g_pawn_storage + kBoolBase + kBoolStride * i); }

static uint32_t flags_of() {
    uint32_t flags = 0;
    for (int i = 0; i < kBoolCount; ++i) flags |= (*harry_word(i) & 1u) << i;
    return flags;
}

// Applies one context: class walk mode, Harry bit words, state chain, boss.
static void configure(int klass, uint32_t bits, int state, int modern) {
    g_obj_class = g_base;
    g_base_super = NULL;
    g_missing_mask = 0;
    switch (klass) {
    case 0: g_obj_class = g_harry; break;
    case 1: g_base_super = g_harry; break;
    case 2: g_base_super = NULL; break;
    default:
        g_base_super = g_harry2;
        g_missing_mask = 1u << 3;
        break;
    }
    g_walk_present = state != 1;
    g_cam_present = state != 2;
    g_boss_set = state == 3;
    memset(g_pawn_storage, 0, sizeof g_pawn_storage);
    APlayerPawn* p = (APlayerPawn*)g_pawn_storage;
    p->bModernThirdPersonControls = modern;
    p->ViewTarget = (AActor*)g_view_storage;
    ((AActor*)g_view_storage)->Rotation.Yaw = 0;
    for (int i = 0; i < kBoolCount; ++i) *harry_word(i) = (bits >> i) & 1u;
    if (g_boss_set) *(UObject**)(g_pawn_storage + kBoolBase + kBoolStride * kBoolCount) = (UObject*)g_view_storage;
}

int main() {
    for (int i = 0; i < kBoolCount; ++i) {
        UBoolProperty* bp = (UBoolProperty*)g_prop_storage[i];
        bp->Offset = kBoolBase + kBoolStride * i;
        bp->BitMask = 1;
    }
    ((UObjectProperty*)g_prop_storage[kBoolCount])->Offset = kBoolBase + kBoolStride * kBoolCount;

    int ctx = 0;
    // Context 128 disables modern controls; the other 128 cover klass x bits x state.
    for (int pass = 0; pass < 129; ++pass) {
        const int modern = pass < 128;
        const int klass = modern ? (pass / 32) : 0;
        const int bits_idx = modern ? ((pass / 4) % 8) : 0;
        const int state = modern ? (pass % 4) : 0;
        const uint32_t bits = kBitsVariants[bits_idx];
        const int c = ctx++;

        // Begin: raw refs start at sentinels; the C++ returns before writing them when gated.
        configure(klass, bits, state, modern);
        APlayerPawn* p = (APlayerPawn*)g_pawn_storage;
        p->aBaseX = 0.75f;
        p->aStrafe = -0.5f;
        p->aTurn = 0.25f;
        p->aLookUp = -0.125f;
        float rbx = 7.0f, rs = 7.5f, ryaw = 8.0f, rp = 8.5f;
        int before = g_logs;
        const UBOOL ret = p->BeginModernThirdPersonInput(rbx, rs, ryaw, rp);
        const int dlogs = g_logs - before;
        printf("begin ctx=%03d k=%d bits=%02d state=%d modern=%d ret=%d raw=%08x:%08x:%08x:%08x pawn=%08x:%08x:%08x:%08x flags=%04x logs=%d",
               c, klass, bits_idx, state, modern, ret,
               fbits(rbx), fbits(rs), fbits(ryaw), fbits(rp),
               fbits(p->aBaseX), fbits(p->aStrafe), fbits(p->aTurn), fbits(p->aLookUp),
               flags_of(), dlogs);
        if (dlogs) printf(" missing=%ls", g_last_log);
        printf("\n");

        // Finish: two input vectors, each from a freshly configured pawn.
        static const float kFinish[2][6] = {
            {1.0f, 0.5f, -0.25f, 0.3f, -0.2f, 0.016666668f},
            {1.0f, -0.9f, 0.7f, 12.5f, 3.0f, 0.25f},
        };
        for (int v = 0; v < 2; ++v) {
            configure(klass, bits, state, modern);
            p = (APlayerPawn*)g_pawn_storage;
            p->aForward = 0.6f;
            p->aSideMove = 0.2f;
            p->aStrafe = 0.1f;
            p->aBaseX = 0.3f;
            p->aTurn = 0.0f;
            p->aLookUp = 0.0f;
            p->SmoothMouseX = 0.125f;
            p->SmoothMouseY = -0.5f;
            p->DesiredFOV = 90.0f;
            before = g_logs;
            p->FinishModernThirdPersonInput(
                (UBOOL)kFinish[v][0], kFinish[v][1], kFinish[v][2], kFinish[v][3],
                kFinish[v][4], kFinish[v][5]);
            printf("finish ctx=%03d vec=%d aforward=%08x astrafe=%08x aside=%08x abasex=%08x smoothx=%08x smoothy=%08x flags=%04x logs=%d",
                   c, v, fbits(p->aForward), fbits(p->aStrafe), fbits(p->aSideMove), fbits(p->aBaseX),
                   fbits(p->SmoothMouseX), fbits(p->SmoothMouseY), flags_of(), g_logs - before);
            if (g_logs - before) printf(" missing=%ls", g_last_log);
            printf("\n");
        }

        // Apply: bit 0 (screen-relative) is set so the movement gate can pass.
        configure(klass, bits | 1u, state, modern);
        p = (APlayerPawn*)g_pawn_storage;
        const bool zero_move = bits_idx == 7;
        p->aForward = zero_move ? 0.0f : 0.7f;
        p->aSideMove = zero_move ? 0.0f : -0.4f;
        p->Velocity = FVector(120.0f, -30.0f, 50.0f);
        if (bits_idx == 5) p->Velocity = FVector(0.0f, 0.0f, 0.0f);
        p->Rotation.Yaw = 12345;
        p->DesiredRotation.Yaw = 1111;
        ((AActor*)g_view_storage)->Rotation.Yaw = 9000;
        before = g_logs;
        p->ApplyModernThirdPersonMovement();
        printf("apply ctx=%03d acc=%08x:%08x:%08x desired=%d flags=%04x logs=%d\n",
               c, fbits(p->Acceleration.X), fbits(p->Acceleration.Y), fbits(p->Acceleration.Z),
               p->DesiredRotation.Yaw, flags_of(), g_logs - before);
    }
    return 0;
}
