// Original-object pawn move native fixture harness. gen_pawn_native_move.sh
// rewrites the 18 move natives of the pinned UnPawn.cpp, inside their own
// function spans only, so that every engine leaf they reach (IsA and IsProbing
// tests, reach and path searches, steering, latent state, noise, see-player
// events, the level pawn and navigation lists, Rotation.Vector) calls a PawnMove*
// hook defined here, and P_GET_* argument reads pop from the queue filled by
// the scenario generator below. The decision logic and float math run unmodified
// from the original object.
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
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
// guard() setjmp target. Defined here so setjmp never writes into a nm stub.
jmp_buf __Context::Env;
// operator new/delete (UnFile.h) call GMalloc->Malloc/Free. Core.cpp is not linked, so provide
// a libc-backed allocator (same pattern as palette_harness.cpp).
class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { return malloc( Count ? Count : 1 ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { return realloc( Original, Count ? Count : 1 ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc GHarnessMalloc;
FMalloc* GMalloc = &GHarnessMalloc;
BYTE GRegisterNative(INT, const Native&) { return 0; }

enum { kPool = 4, kCases = 32, kEntries = 18 };
enum { K_REACH, K_CANMOVE, K_FINDTO, K_FINDTOWARD, K_RANDOMDEST, K_LOS, K_ROTATE, K_MOVE, K_KINDS };
enum {
    E_ACTOR_REACHABLE,
    E_FIND_PATH_TO,
    E_FIND_PATH_TOWARD,
    E_FIND_RANDOM_DEST,
    E_MAKE_NOISE,
    E_MOVE_TO,
    E_POLL_MOVE_TO,
    E_MOVE_TOWARD,
    E_POLL_MOVE_TOWARD,
    E_STRAFE_TO,
    E_POLL_STRAFE_TO,
    E_STRAFE_FACING,
    E_POLL_STRAFE_FACING,
    E_TURN_TOWARD,
    E_POLL_TURN_TOWARD,
    E_TURN_TO,
    E_POLL_TURN_TO,
    E_SHOW_SELF,
};
static const char* const kEntryName[kEntries] = {
    "actorReachable", "findPathTo", "findPathToward", "findRandomDest", "makeNoise", "moveTo",
    "pollMoveTo", "moveToward", "pollMoveToward", "strafeTo", "pollStrafeTo", "strafeFacing",
    "pollStrafeFacing", "turnToward", "pollTurnToward", "turnTo", "pollTurnTo", "showSelf",
};

static constexpr size_t kMaxSize(size_t a, size_t b) { return a > b ? a : b; }
static constexpr size_t kObjBytes =
    kMaxSize(kMaxSize(sizeof(APawn), sizeof(ANavigationPoint)), kMaxSize(sizeof(AInventory), sizeof(AActor)));

alignas(16) static char g_pool_storage[kPool][kObjBytes];
alignas(16) static char g_level_storage[sizeof(ULevel)];
alignas(16) static char g_zone_storage[sizeof(AZoneInfo)];
alignas(16) static char g_frame_storage[sizeof(FFrame)];
alignas(16) static char g_result[16];

// Scenario generator: 15-bit LCG shared with the Rust replay test.
static uint32_t g_seed = 0x5eed0001u;
static uint32_t next15() {
    g_seed = g_seed * 1664525u + 1013904223u;
    return (g_seed >> 16) & 0x7fffu;
}
static uint32_t ri(uint32_t n) { return next15() % n; }
// Divided by 3: products of these values are inexact in f32, so FMA contraction changes the bits.
static FLOAT frand() { return ((FLOAT)(next15() & 4095u) / 256.0f - 8.0f) / 3.0f; }

static INT g_netmode, g_reach_count, g_water;
static uint32_t g_isa[kPool], g_probe[kPool];
static int g_nav_head, g_pawn_head;
static uint32_t g_script[K_KINDS];
static int g_calls[K_KINDS];
static INT g_latent;
static FVector g_rotvec;
static std::string g_trace;

struct ArgSlot {
    char kind;
    int present;
    int pool;
    INT ival;
    FLOAT fval;
    FVector vec;
};
static ArgSlot g_args[4];
static int g_arg_n, g_arg_pos;

static AActor* pool_actor(int i) { return (AActor*)g_pool_storage[i]; }
static APawn* pool_pawn(int i) { return (APawn*)g_pool_storage[i]; }
static int pool_index(const void* p) {
    if (!p) return -1;
    for (int i = 0; i < kPool; ++i)
        if (p == (const void*)g_pool_storage[i]) return i;
    abort();
}
static int must_index(const void* p) {
    const int i = pool_index(p);
    if (i < 0) abort();
    return i;
}
static std::string id_idx(int i) { return i < 0 ? std::string("-") : "p" + std::to_string(i); }
static std::string id_of(const void* p) { return id_idx(pool_index(p)); }
static uint32_t fbits(FLOAT f) {
    uint32_t u;
    memcpy(&u, &f, sizeof u);
    return u;
}
static std::string hex32(uint32_t v) {
    char b[16];
    snprintf(b, sizeof b, "%08x", (unsigned)v);
    return b;
}
static std::string hexf(FLOAT f) { return hex32(fbits(f)); }
static std::string xyz(const FVector& v) { return hexf(v.X) + "," + hexf(v.Y) + "," + hexf(v.Z); }
static void tr(const std::string& tok) {
    if (!g_trace.empty()) g_trace += ';';
    g_trace += tok;
}
// Scripted bit for the k-th call of a hook kind in this case.
static INT next_call(int kind, int* k_out) {
    const int k = g_calls[kind]++;
    if (k_out) *k_out = k;
    return (INT)((g_script[kind] >> (k & 31)) & 1u);
}

// Engine leaf hooks (rewritten calls inside the 18 move native spans).
INT PawnMoveIsA(const AActor* obj, INT kind) {
    const int i = must_index(obj);
    const INT r = (INT)((g_isa[i] >> kind) & 1u);
    tr("isA(" + id_idx(i) + "," + std::to_string(kind) + ")=" + std::to_string(r));
    return r;
}
UBOOL PawnMoveIsProbing(const AActor* obj, INT name) {
    const int i = must_index(obj);
    const UBOOL r = (UBOOL)((g_probe[i] >> name) & 1u);
    tr("probing(" + id_idx(i) + "," + std::to_string(name) + ")=" + std::to_string(r));
    return r;
}
INT PawnMoveNetMode(const AActor*) {
    tr("netmode()=" + std::to_string(g_netmode));
    return g_netmode;
}
INT PawnMoveReachSpecCount(const AActor*) {
    tr("reachspecs()=" + std::to_string(g_reach_count));
    return g_reach_count;
}
ANavigationPoint* PawnMoveLevelNavList(const AActor*) {
    tr("navlist()=" + id_idx(g_nav_head));
    return g_nav_head < 0 ? NULL : (ANavigationPoint*)g_pool_storage[g_nav_head];
}
APawn* PawnMoveLevelPawnList(const AActor*) {
    tr("pawnlist()=" + id_idx(g_pawn_head));
    return g_pawn_head < 0 ? NULL : pool_pawn(g_pawn_head);
}
INT PawnMoveActorReachable(const AActor*, const AActor* actor) {
    const INT r = next_call(K_REACH, nullptr);
    tr("reach(" + id_idx(must_index(actor)) + ")=" + std::to_string(r));
    return r;
}
INT PawnMoveCanMoveTo(const AActor*, const AActor* anchor, const AActor* dest) {
    const INT r = next_call(K_CANMOVE, nullptr);
    tr("canmove(" + id_idx(must_index(anchor)) + "," + id_idx(must_index(dest)) + ")=" + std::to_string(r));
    return r;
}
INT PawnMoveFindPathTo(const AActor*, const FVector* point, INT single, INT clear, AActor** best) {
    int k = 0;
    const INT r = next_call(K_FINDTO, &k);
    *best = r ? pool_actor(1 + k % 3) : NULL;
    tr("findto(" + xyz(*point) + "," + std::to_string(single) + "," + std::to_string(clear) + ")=" +
       std::to_string(r) + ":" + id_of(*best));
    return r;
}
INT PawnMoveFindPathToward(const AActor*, const AActor* goal, INT single, INT clear, AActor** best) {
    int k = 0;
    const INT r = next_call(K_FINDTOWARD, &k);
    *best = r ? pool_actor(1 + k % 3) : NULL;
    tr("findtoward(" + id_idx(must_index(goal)) + "," + std::to_string(single) + "," + std::to_string(clear) +
       ")=" + std::to_string(r) + ":" + id_of(*best));
    return r;
}
INT PawnMoveFindRandomDest(const AActor*, AActor** best) {
    int k = 0;
    const INT r = next_call(K_RANDOMDEST, &k);
    *best = r ? pool_actor(1 + k % 3) : NULL;
    tr("randomdest()=" + std::to_string(r) + ":" + id_of(*best));
    return r;
}
void PawnMoveClearPaths(const AActor*) { tr("clear()"); }
void PawnMoveHandleSpecial(const AActor*, AActor** best) { tr("special(" + id_idx(must_index(*best)) + ")"); }
void PawnMoveSetMoveTimer(const AActor*, FLOAT t) { tr("timer(" + hexf(t) + ")"); }
INT PawnMoveRotateToward(const AActor*, const FVector* focal) {
    const INT r = next_call(K_ROTATE, nullptr);
    tr("rotate(" + xyz(*focal) + ")=" + std::to_string(r));
    return r;
}
INT PawnMoveMoveToward(const AActor*, const FVector* dest) {
    const INT r = next_call(K_MOVE, nullptr);
    tr("move(" + xyz(*dest) + ")=" + std::to_string(r));
    return r;
}
void PawnMoveSetLatent(const AActor*, INT value) {
    g_latent = value;
    tr("latent(" + std::to_string(value) + ")");
}
void PawnMoveEventAlterDestination(APawn* self) {
    self->Destination.X += 3.0f;
    tr("alter()");
}
void PawnMoveEventSeePlayer(const AActor* pawn, const AActor* seer) {
    tr("seeplayer(" + id_idx(must_index(pawn)) + "," + id_idx(must_index(seer)) + ")");
}
DWORD PawnMoveLineOfSightTo(const AActor* pawn, const AActor* other, INT show) {
    const INT r = next_call(K_LOS, nullptr);
    tr("los(" + id_idx(must_index(pawn)) + "," + id_idx(must_index(other)) + "," + std::to_string(show) +
       ")=" + std::to_string(r));
    return (DWORD)r;
}
void PawnMoveCheckNoiseHearing(const AActor*, FLOAT loudness) { tr("noise(" + hexf(loudness) + ")"); }
FVector PawnMoveRotationVector(const AActor*) {
    tr("rotvec()=" + xyz(g_rotvec));
    return g_rotvec;
}

// Argument queue: the P_GET_* replacements pop in the order the original reads.
static ArgSlot& pop_slot() {
    if (g_arg_pos >= g_arg_n) abort();
    return g_args[g_arg_pos++];
}
FVector PawnMovePopVector() { return pop_slot().vec; }
AActor* PawnMovePopActor() {
    const ArgSlot& s = pop_slot();
    return s.pool < 0 ? NULL : pool_actor(s.pool);
}
FLOAT PawnMovePopFloat() { return pop_slot().fval; }
FLOAT PawnMovePopFloatOptx(FLOAT def) {
    const ArgSlot& s = pop_slot();
    return s.present ? s.fval : def;
}
INT PawnMovePopIntOptx(INT def) {
    const ArgSlot& s = pop_slot();
    return s.present ? s.ival : def;
}
UBOOL PawnMovePopUboolOptx(UBOOL def) {
    const ArgSlot& s = pop_slot();
    return s.present ? (UBOOL)s.ival : def;
}

static void push_vec() {
    ArgSlot& s = g_args[g_arg_n++];
    s.kind = 'V';
    s.vec.X = frand();
    s.vec.Y = frand();
    s.vec.Z = frand();
}
static void push_actor(bool nullable) {
    ArgSlot& s = g_args[g_arg_n++];
    s.kind = 'A';
    if (nullable) {
        const uint32_t pick = ri(5);
        s.pool = pick == 4 ? -1 : (int)pick;
    } else {
        s.pool = (int)ri(4);
    }
}
static void push_float() {
    ArgSlot& s = g_args[g_arg_n++];
    s.kind = 'F';
    s.fval = frand();
}
static void push_opt_int() {
    ArgSlot& s = g_args[g_arg_n++];
    s.kind = 'I';
    s.present = (int)ri(2);
    s.ival = (INT)ri(2);
}
static void push_opt_bool() {
    ArgSlot& s = g_args[g_arg_n++];
    s.kind = 'U';
    s.present = (int)ri(2);
    s.ival = (INT)ri(2);
}
static void push_opt_float() {
    ArgSlot& s = g_args[g_arg_n++];
    s.kind = 'G';
    s.present = (int)ri(2);
    s.fval = frand();
}
static void queue_for(int e) {
    g_arg_n = 0;
    switch (e) {
    case E_ACTOR_REACHABLE: push_actor(false); break;
    case E_FIND_PATH_TO: push_vec(); push_opt_int(); push_opt_bool(); break;
    case E_FIND_PATH_TOWARD: push_actor(false); push_opt_int(); push_opt_bool(); break;
    case E_FIND_RANDOM_DEST: push_opt_bool(); break;
    case E_MAKE_NOISE: push_float(); break;
    case E_MOVE_TO: push_vec(); push_opt_float(); break;
    case E_MOVE_TOWARD: push_actor(true); push_opt_float(); break;
    case E_STRAFE_TO: push_vec(); push_vec(); break;
    case E_STRAFE_FACING: push_vec(); push_actor(true); break;
    case E_TURN_TOWARD: push_actor(true); break;
    case E_TURN_TO: push_vec(); break;
    default: break;
    }
}
static std::string render_slot(const ArgSlot& s) {
    switch (s.kind) {
    case 'V': return "v=" + xyz(s.vec);
    case 'A': return "a=" + id_idx(s.pool);
    case 'F': return "f=" + hexf(s.fval);
    case 'I': return "i=" + std::to_string(s.present) + ":" + std::to_string(s.ival);
    case 'U': return "u=" + std::to_string(s.present) + ":" + std::to_string(s.ival);
    case 'G': return "g=" + std::to_string(s.present) + ":" + hexf(s.fval);
    default: abort();
    }
}

static AActor* pick_target() {
    const uint32_t pick = ri(5);
    return pick == 4 ? NULL : pool_actor((int)pick);
}

static void setup_case() {
    static const int kLatent[3] = {0, 501, 511};
    static const int kPhys[5] = {1, 3, 4, 10, 2};
    memset(g_calls, 0, sizeof g_calls);
    g_netmode = (INT)ri(4);
    g_reach_count = (INT)ri(3);
    for (int i = 0; i < kPool; ++i) g_isa[i] = ri(8);
    for (int i = 0; i < kPool; ++i) g_probe[i] = ri(4);
    g_water = (INT)ri(2);
    memset(g_pool_storage, 0, sizeof g_pool_storage);
    memset(g_level_storage, 0, sizeof g_level_storage);
    memset(g_zone_storage, 0, sizeof g_zone_storage);
    ((AZoneInfo*)g_zone_storage)->bWaterZone = g_water;
    for (int i = 0; i < kPool; ++i) {
        AActor* a = pool_actor(i);
        FVector loc;
        loc.X = frand();
        loc.Y = frand();
        loc.Z = frand();
        const FLOAT radius = (FLOAT)ri(160) / 3.0f;
        const FLOAT height = (FLOAT)ri(160) / 3.0f;
        const int phys = kPhys[ri(5)];
        const uint32_t marker = ri(4);
        a->XLevel = (ULevel*)g_level_storage;
        a->Region.Zone = (decltype(a->Region.Zone))g_zone_storage;
        a->Physics = (decltype(a->Physics))phys;
        a->Location = loc;
        a->CollisionRadius = radius;
        a->CollisionHeight = height;
        ((AInventory*)a)->MyMarker =
            marker == 0 ? NULL : (decltype(((AInventory*)a)->MyMarker))pool_actor((int)marker);
    }

    APawn* pw = pool_pawn(0);
    pw->bIsPlayer = ri(2);
    pw->bCanSwim = ri(2);
    pw->bCanStrafe = ri(2);
    pw->bReducedSpeed = ri(2);
    pw->bAdvancedTactics = ri(2);
    pw->bShootSpecial = ri(2);
    pw->SpecialPause = frand();
    pw->MaxDesiredSpeed = frand();
    pw->GroundSpeed = frand();
    pw->AccelRate = frand();
    pw->DesiredSpeed = frand();
    pw->MoveTimer = frand();
    pw->SightCounter = frand();
    pw->Floor.X = frand();
    pw->Floor.Y = frand();
    pw->Floor.Z = frand();
    AActor* self = pool_actor(0);
    self->Acceleration.X = frand();
    self->Acceleration.Y = frand();
    self->Acceleration.Z = frand();
    pw->Destination.X = frand();
    pw->Destination.Y = frand();
    pw->Destination.Z = frand();
    pw->Focus.X = frand();
    pw->Focus.Y = frand();
    pw->Focus.Z = frand();
    pw->MoveTarget = pick_target();
    pw->FaceTarget = pick_target();
    pw->SpecialGoal = pick_target();

    const uint32_t nav_mask = ri(16);
    const uint32_t pawn_mask = ri(16);
    g_nav_head = -1;
    g_pawn_head = -1;
    int prev = -1;
    for (int i = 0; i < kPool; ++i) {
        if (!((nav_mask >> i) & 1u)) continue;
        if (prev < 0)
            g_nav_head = i;
        else
            ((ANavigationPoint*)g_pool_storage[prev])->nextNavigationPoint = (ANavigationPoint*)g_pool_storage[i];
        prev = i;
    }
    prev = -1;
    for (int i = 0; i < kPool; ++i) {
        if (!((pawn_mask >> i) & 1u)) continue;
        if (prev < 0)
            g_pawn_head = i;
        else
            ((APawn*)g_pool_storage[prev])->nextPawn = pool_pawn(i);
        prev = i;
    }

    for (int k = 0; k < K_KINDS; ++k) {
        const uint32_t hi = next15();
        const uint32_t lo = next15();
        g_script[k] = (hi << 15) ^ lo;
    }
    g_rotvec.X = frand();
    g_rotvec.Y = frand();
    g_rotvec.Z = frand();
    g_latent = kLatent[ri(3)];
}

static void run_entry(int e, int c) {
    queue_for(e);
    std::string args;
    for (int j = 0; j < g_arg_n; ++j) {
        if (j) args += ' ';
        args += render_slot(g_args[j]);
    }
    g_arg_pos = 0;
    g_trace.clear();
    memset(g_result, 0, sizeof g_result);
    FFrame& st = *(FFrame*)g_frame_storage;
    APawn* p = pool_pawn(0);
    switch (e) {
    case E_ACTOR_REACHABLE: p->execactorReachable(st, g_result); break;
    case E_FIND_PATH_TO: p->execFindPathTo(st, g_result); break;
    case E_FIND_PATH_TOWARD: p->execFindPathToward(st, g_result); break;
    case E_FIND_RANDOM_DEST: p->execFindRandomDest(st, g_result); break;
    case E_MAKE_NOISE: pool_actor(0)->execMakeNoise(st, g_result); break;
    case E_MOVE_TO: p->execMoveTo(st, g_result); break;
    case E_POLL_MOVE_TO: p->execPollMoveTo(st, g_result); break;
    case E_MOVE_TOWARD: p->execMoveToward(st, g_result); break;
    case E_POLL_MOVE_TOWARD: p->execPollMoveToward(st, g_result); break;
    case E_STRAFE_TO: p->execStrafeTo(st, g_result); break;
    case E_POLL_STRAFE_TO: p->execPollStrafeTo(st, g_result); break;
    case E_STRAFE_FACING: p->execStrafeFacing(st, g_result); break;
    case E_POLL_STRAFE_FACING: p->execPollStrafeFacing(st, g_result); break;
    case E_TURN_TOWARD: p->execTurnToward(st, g_result); break;
    case E_POLL_TURN_TOWARD: p->execPollTurnToward(st, g_result); break;
    case E_TURN_TO: p->execTurnTo(st, g_result); break;
    case E_POLL_TURN_TO: p->execPollTurnTo(st, g_result); break;
    case E_SHOW_SELF: p->ShowSelf(); break;
    default: abort();
    }
    if (g_arg_pos != g_arg_n) abort();

    std::string ret;
    switch (e) {
    case E_ACTOR_REACHABLE:
    case E_MOVE_TO:
    case E_POLL_MOVE_TO:
    case E_MOVE_TOWARD:
    case E_POLL_MOVE_TOWARD:
    case E_STRAFE_TO:
    case E_POLL_STRAFE_TO:
    case E_STRAFE_FACING:
    case E_POLL_STRAFE_FACING:
    case E_TURN_TOWARD:
    case E_POLL_TURN_TOWARD:
    case E_TURN_TO:
    case E_POLL_TURN_TO: ret = hex32(*(DWORD*)g_result); break;
    case E_FIND_PATH_TO:
    case E_FIND_PATH_TOWARD:
    case E_FIND_RANDOM_DEST: ret = id_of(*(AActor**)g_result); break;
    default: ret = "-"; break;
    }
    std::string line = std::string(kEntryName[e]) + " c=" + std::to_string(c);
    if (!args.empty()) line += " " + args;
    line += " ret=" + ret;
    line += " lat=" + std::to_string(g_latent);
    line += " mt=" + id_of(p->MoveTarget) + " fa=" + id_of(p->FaceTarget) + " sg=" + id_of(p->SpecialGoal);
    line += " dest=" + xyz(p->Destination) + " focus=" + xyz(p->Focus) + " acc=" + xyz(pool_actor(0)->Acceleration);
    line += " ds=" + hexf(p->DesiredSpeed) + " mtm=" + hexf(p->MoveTimer);
    line += " red=" + std::to_string((int)p->bReducedSpeed) + " sp=" + hexf(p->SpecialPause) +
            " shoot=" + std::to_string((int)p->bShootSpecial);
    line += " calls=" + g_trace;
    printf("%s\n", line.c_str());
}

int main() {
    for (int c = 0; c < kCases; ++c) {
        setup_case();
        for (int e = 0; e < kEntries; ++e) run_entry(e, c);
    }
    return 0;
}
