// Original-object AI native fixture harness. gen_pawn_native_ai.sh rewrites the
// engine reads, writes and service calls of the ten ai natives in the pinned
// UnPawn.cpp to the PawnAi* hooks defined here. Each hook logs its arguments and
// returns scripted state; the native control flow and float math run unmodified.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
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

// Inputs of the ai natives that the rewritten UnPawn.cpp reads from globals
// in place of FFrame bytecode (P_GET_* macros).
FLOAT g_ai_best_aim, g_ai_best_dist, g_ai_delta_time;
FVector g_ai_fire_dir, g_ai_proj_start;

enum {
    kPawns = 6,
    kActors = 4,
    kLevelId = kPawns + kActors + 1,
    kPris = 2,
    kIdSlots = 16,
    kWorldWords = kPawns * 16 + kActors * 3,
};
enum { kTraceSlots = 8 };

// Hook kinds, one per Hp2PawnNativeAiServices field.
enum {
    K_ROTATION_VECTOR = 1,
    K_STATE_LATENT_ACTION,
    K_SET_STATE_LATENT_ACTION,
    K_LEVEL_NET_MODE,
    K_LEVEL_PAWN_LIST,
    K_SET_LEVEL_PAWN_LIST,
    K_LEVEL_TEAM_GAME,
    K_LEVEL_ACTOR_COUNT,
    K_LEVEL_ACTOR,
    K_VIEWPORT_COUNT,
    K_VIEWPORT_ACTOR,
    K_ACTOR_NET_TEMPORARY,
    K_ACTOR_PROJ_TARGET,
    K_ACTOR_IS_PAWN,
    K_PAWN_HEALTH,
    K_PAWN_BEHIND_VIEW,
    K_REPLICATION_TEAM,
    K_LINE_OF_SIGHT_TO,
    K_SINGLE_LINE_CHECK,
    K_EVENT_LONG_FALL,
};

// Object storage has no constructors: the natives only read and write fields.
alignas(16) static char g_pawn_store[kPawns][sizeof(APlayerPawn)];
alignas(16) static char g_actor_store[kActors][sizeof(AActor)];
alignas(16) static char g_level_store[sizeof(ULevel)];
alignas(16) static char g_pri_store[kPris][sizeof(APlayerReplicationInfo)];
alignas(16) static char g_frame_store[sizeof(FFrame)];
alignas(16) static BYTE g_code[16];

// Object ids: 1..6 pawns (APlayerPawn), 7..10 actors, 11 level, 12..13 PRIs; 0 is null.
static APlayerPawn* pawn_at(uint32_t id) { return (APlayerPawn*)g_pawn_store[id - 1]; }
static AActor* actor_at(uint32_t id) { return (AActor*)g_actor_store[id - 1 - kPawns]; }
static ULevel* level_at() { return (ULevel*)g_level_store; }
static APlayerReplicationInfo* pri_at(uint32_t id) {
    return (APlayerReplicationInfo*)g_pri_store[id - kLevelId - 1];
}
static void* ptr_of(uint32_t id) {
    if (id == 0) return NULL;
    if (id <= kPawns) return pawn_at(id);
    if (id <= kPawns + kActors) return actor_at(id);
    if (id == kLevelId) return level_at();
    if (id <= kLevelId + kPris) return pri_at(id);
    abort();
}
static uint32_t object_id(const void* p) {
    if (p == NULL) return 0;
    for (uint32_t id = 1; id <= kLevelId + kPris; ++id)
        if (ptr_of(id) == p) return id;
    abort();
}
static uint32_t bits(float f) {
    uint32_t u;
    memcpy(&u, &f, 4);
    return u;
}
static uint32_t wrd(INT v) { return (uint32_t)v; }

// Scripted hook state, indexed by object id.
static INT g_net_mode, g_team_game, g_actor_count, g_viewport_count;
static uint32_t g_pawn_head;
static uint32_t g_actor_list[kIdSlots], g_viewport_list[kIdSlots];
static INT g_latent[kIdSlots], g_health[kIdSlots], g_team[kIdSlots];
static UBOOL g_net_temp[kIdSlots], g_proj[kIdSlots], g_is_pawn[kIdSlots], g_behind[kIdSlots];
static UBOOL g_los[kIdSlots][kIdSlots];
static FLOAT g_trace_time[kTraceSlots];
static uint32_t g_trace_at;

// Hook call log: kind, argc, args..., outc, outs... per entry.
static uint32_t g_log[4096];
static uint32_t g_log_len, g_log_count;
static void record_log(uint32_t kind, uint32_t argc, const uint32_t* args, uint32_t outc,
                       const uint32_t* outs) {
    g_log[g_log_len++] = kind;
    g_log[g_log_len++] = argc;
    for (uint32_t i = 0; i < argc; ++i) g_log[g_log_len++] = args[i];
    g_log[g_log_len++] = outc;
    for (uint32_t i = 0; i < outc; ++i) g_log[g_log_len++] = outs[i];
    g_log_count++;
}
static INT scripted(uint32_t kind, uint32_t argc, const uint32_t* args, INT value) {
    const uint32_t outs[] = {wrd(value)};
    record_log(kind, argc, args, 1, outs);
    return value;
}

// Hooks called from the rewritten UnPawn.cpp.
FVector PawnAiRotationVector(const FRotator& Rot) {
    FRotator copy = Rot;
    const FVector v = copy.Vector();
    const uint32_t args[] = {wrd(Rot.Pitch), wrd(Rot.Yaw), wrd(Rot.Roll)};
    const uint32_t outs[] = {bits(v.X), bits(v.Y), bits(v.Z)};
    record_log(K_ROTATION_VECTOR, 3, args, 3, outs);
    return v;
}
INT PawnAiStateLatentAction(AActor* Actor) {
    const uint32_t args[] = {object_id(Actor)};
    return scripted(K_STATE_LATENT_ACTION, 1, args, g_latent[args[0]]);
}
void PawnAiSetStateLatentAction(AActor* Actor, INT Action) {
    const uint32_t id = object_id(Actor);
    g_latent[id] = Action;
    const uint32_t args[] = {id, wrd(Action)};
    record_log(K_SET_STATE_LATENT_ACTION, 2, args, 0, NULL);
}
INT PawnAiLevelNetMode(ULevel* Level) {
    const uint32_t args[] = {object_id(Level)};
    return scripted(K_LEVEL_NET_MODE, 1, args, g_net_mode);
}
APawn* PawnAiLevelPawnList(ULevel* Level) {
    const uint32_t args[] = {object_id(Level)};
    const uint32_t outs[] = {g_pawn_head};
    record_log(K_LEVEL_PAWN_LIST, 1, args, 1, outs);
    return (APawn*)ptr_of(g_pawn_head);
}
void PawnAiSetLevelPawnList(ULevel* Level, APawn* Pawn) {
    g_pawn_head = object_id(Pawn);
    const uint32_t args[] = {object_id(Level), g_pawn_head};
    record_log(K_SET_LEVEL_PAWN_LIST, 2, args, 0, NULL);
}
INT PawnAiLevelTeamGame(ULevel* Level) {
    const uint32_t args[] = {object_id(Level)};
    return scripted(K_LEVEL_TEAM_GAME, 1, args, g_team_game);
}
INT PawnAiLevelActorCount(ULevel* Level) {
    const uint32_t args[] = {object_id(Level)};
    return scripted(K_LEVEL_ACTOR_COUNT, 1, args, g_actor_count);
}
AActor* PawnAiLevelActor(ULevel* Level, INT Index) {
    const uint32_t args[] = {object_id(Level), wrd(Index)};
    const uint32_t outs[] = {g_actor_list[Index]};
    record_log(K_LEVEL_ACTOR, 2, args, 1, outs);
    return (AActor*)ptr_of(g_actor_list[Index]);
}
INT PawnAiViewportCount(ULevel* Level) {
    const uint32_t args[] = {object_id(Level)};
    return scripted(K_VIEWPORT_COUNT, 1, args, g_viewport_count);
}
APlayerPawn* PawnAiViewportActor(ULevel* Level, INT Index) {
    const uint32_t args[] = {object_id(Level), wrd(Index)};
    const uint32_t outs[] = {g_viewport_list[Index]};
    record_log(K_VIEWPORT_ACTOR, 2, args, 1, outs);
    return (APlayerPawn*)ptr_of(g_viewport_list[Index]);
}
UBOOL PawnAiActorNetTemporary(AActor* Actor) {
    const uint32_t args[] = {object_id(Actor)};
    return scripted(K_ACTOR_NET_TEMPORARY, 1, args, g_net_temp[args[0]]);
}
UBOOL PawnAiActorProjTarget(AActor* Actor) {
    const uint32_t args[] = {object_id(Actor)};
    return scripted(K_ACTOR_PROJ_TARGET, 1, args, g_proj[args[0]]);
}
UBOOL PawnAiActorIsPawn(AActor* Actor) {
    const uint32_t args[] = {object_id(Actor)};
    return scripted(K_ACTOR_IS_PAWN, 1, args, g_is_pawn[args[0]]);
}
INT PawnAiPawnHealth(APawn* Pawn) {
    const uint32_t args[] = {object_id(Pawn)};
    return scripted(K_PAWN_HEALTH, 1, args, g_health[args[0]]);
}
UBOOL PawnAiPawnBehindView(APlayerPawn* Viewer) {
    const uint32_t args[] = {object_id(Viewer)};
    return scripted(K_PAWN_BEHIND_VIEW, 1, args, g_behind[args[0]]);
}
INT PawnAiReplicationTeam(APlayerReplicationInfo* Info) {
    const uint32_t args[] = {object_id(Info)};
    return scripted(K_REPLICATION_TEAM, 1, args, g_team[args[0]]);
}
UBOOL PawnAiLineOfSightTo(APawn* Pawn, AActor* Other) {
    const uint32_t a = object_id(Pawn), b = object_id(Other);
    const uint32_t args[] = {a, b};
    const uint32_t outs[] = {wrd(g_los[a][b])};
    record_log(K_LINE_OF_SIGHT_TO, 2, args, 1, outs);
    return g_los[a][b];
}
void PawnAiSingleLineCheck(ULevel* Level, FCheckResult& Hit, AActor* Source, const FVector& End,
                           const FVector& Start, DWORD Trace, const FVector& Extent) {
    const FLOAT time = g_trace_time[g_trace_at % kTraceSlots];
    g_trace_at++;
    const uint32_t args[] = {object_id(Level), object_id(Source), bits(End.X), bits(End.Y),
                             bits(End.Z), bits(Start.X), bits(Start.Y), bits(Start.Z),
                             Trace, bits(Extent.X), bits(Extent.Y), bits(Extent.Z)};
    const uint32_t outs[] = {bits(time), bits(0.f), bits(0.f), bits(1.f)};
    record_log(K_SINGLE_LINE_CHECK, 12, args, 4, outs);
    Hit.Time = time;
    Hit.Normal = FVector(0.f, 0.f, 1.f);
}
void PawnAiEventLongFall(APawn* Pawn) {
    const uint32_t args[] = {object_id(Pawn)};
    record_log(K_EVENT_LONG_FALL, 1, args, 0, NULL);
}

// Record layout: entry, case, world-in, args, ret, outs, world-post, nlog, log entries.
static uint32_t g_rng = 0x9e3779b9u;
static uint32_t rnd(uint32_t n) {
    g_rng = g_rng * 1664525u + 1013904223u;
    return ((g_rng >> 8) & 0xffffffu) % n;
}
static FLOAT rndf(int lo, int hi) { return (FLOAT)(lo + (int)rnd((uint32_t)(hi - lo + 1))); }

static uint32_t* pack_vec(uint32_t* w, const FVector& v) {
    w[0] = bits(v.X);
    w[1] = bits(v.Y);
    w[2] = bits(v.Z);
    return w + 3;
}
static void pack_pawn(APlayerPawn* p, uint32_t* w) {
    uint32_t* o = pack_vec(w, p->Location);
    *o++ = bits(p->CollisionRadius);
    *o++ = bits(p->CollisionHeight);
    *o++ = bits(p->LatentFloat);
    *o++ = p->Physics;
    *o++ = p->Role;
    *o++ = object_id(p->XLevel);
    *o++ = object_id(p->nextPawn);
    *o++ = object_id(p->PlayerReplicationInfo);
    *o++ = object_id(p->ViewTarget);
    *o++ = wrd(p->ViewRotation.Pitch);
    *o++ = wrd(p->ViewRotation.Yaw);
    *o++ = wrd(p->ViewRotation.Roll);
    *o++ = bits(p->BaseEyeHeight);
}
static void dump_world(uint32_t* w) {
    for (uint32_t i = 0; i < kPawns; ++i) pack_pawn(pawn_at(i + 1), w + i * 16);
    uint32_t* o = w + kPawns * 16;
    for (uint32_t j = 0; j < kActors; ++j) o = pack_vec(o, actor_at(kPawns + 1 + j)->Location);
}

static void build_world() {
    memset(g_pawn_store, 0, sizeof g_pawn_store);
    memset(g_actor_store, 0, sizeof g_actor_store);
    memset(g_level_store, 0, sizeof g_level_store);
    memset(g_pri_store, 0, sizeof g_pri_store);
    static const FLOAT kHeights[] = {0.f, 32.f, 64.f, 128.f};
    static const FLOAT kRadius[] = {20.f, 34.f, 48.f, 60.f};
    static const FLOAT kCHeight[] = {40.f, 44.f, 60.f, 88.f};
    static const FLOAT kLatent[] = {-1.f, 0.f, 0.5f, 2.5f, -0.5f};
    static const FLOAT kEye[] = {0.f, 20.f, 40.f, 48.f, 64.f};
    static const INT kPitch[] = {0, 8192, -8192, 16384, 40000, 49152, 65000, 32768, 32767, -32000};
    static const BYTE kPhys[] = {0, 1, 2, 2, 3};
    static const BYTE kRole[] = {4, 3, 2};
    static const INT kLatentAction[] = {0, 384, 528, 1};
    static const INT kHealth[] = {0, 1, 100};
    static const FLOAT kTrace[] = {1.f, 1.f, 0.5f, 0.25f, 0.f, 0.9f, 0.75f, 0.999f};
    for (uint32_t i = 0; i < kPawns; ++i) {
        APlayerPawn* p = pawn_at(i + 1);
        p->Location = FVector(rndf(-200, 200), rndf(-200, 200), kHeights[rnd(4)]);
        p->CollisionRadius = kRadius[rnd(4)];
        p->CollisionHeight = kCHeight[rnd(4)];
        p->LatentFloat = kLatent[rnd(5)];
        p->Physics = kPhys[rnd(5)];
        p->Role = kRole[rnd(3)];
        p->XLevel = level_at();
        p->nextPawn = (i + 1 < kPawns && rnd(3) != 0) ? pawn_at(i + 2) : NULL;
        p->PlayerReplicationInfo = rnd(3) == 0 ? NULL : pri_at(kLevelId + 1 + rnd(kPris));
        p->ViewTarget = rnd(3) == 0 ? NULL : (AActor*)ptr_of(1 + rnd(kPawns + kActors));
        p->ViewRotation.Pitch = kPitch[rnd(10)];
        p->ViewRotation.Yaw = (INT)rnd(65536);
        p->ViewRotation.Roll = 0;
        p->BaseEyeHeight = kEye[rnd(5)];
    }
    for (uint32_t j = 0; j < kActors; ++j) {
        AActor* a = actor_at(kPawns + 1 + j);
        a->Location = FVector(rndf(-200, 200), rndf(-200, 200), kHeights[rnd(4)]);
        a->XLevel = level_at();
    }
    g_net_mode = (INT)rnd(4);
    g_team_game = (INT)rnd(2);
    g_pawn_head = rnd(3) == 0 ? 0 : 1 + rnd(kPawns);
    g_actor_count = (INT)rnd(5);
    g_viewport_count = (INT)rnd(4);
    for (uint32_t k = 0; k < kIdSlots; ++k) {
        g_actor_list[k] = rnd(kPawns + kActors + 1);
        g_viewport_list[k] = 1 + rnd(kPawns);
    }
    for (uint32_t id = 1; id <= kPawns + kActors; ++id) {
        g_net_temp[id] = rnd(3) == 0;
        g_proj[id] = rnd(4) != 0;
        g_is_pawn[id] = id <= kPawns;
        g_latent[id] = kLatentAction[rnd(4)];
        g_health[id] = kHealth[rnd(3)];
        g_behind[id] = rnd(2);
    }
    for (uint32_t id = kLevelId + 1; id <= kLevelId + kPris; ++id) g_team[id] = (INT)rnd(3);
    for (uint32_t a = 0; a < kIdSlots; ++a)
        for (uint32_t b = 0; b < kIdSlots; ++b) g_los[a][b] = rnd(2);
    for (uint32_t k = 0; k < kTraceSlots; ++k) g_trace_time[k] = kTrace[rnd(8)];
}

static uint32_t g_case_no;
static uint32_t g_in[kWorldWords];
static void begin_record() {
    dump_world(g_in);
    g_log_len = 0;
    g_log_count = 0;
    g_trace_at = 0;
}
static void put_words(const uint32_t* w, size_t n) {
    for (size_t i = 0; i < n; ++i) printf(" %08x", w[i]);
}
static void end_record(const char* name, const uint32_t* args, size_t nargs, const uint32_t* ret,
                       size_t nret, const uint32_t* outs, size_t nouts) {
    uint32_t post[kWorldWords];
    dump_world(post);
    printf("%s %08x", name, g_case_no++);
    put_words(g_in, kWorldWords);
    put_words(args, nargs);
    put_words(ret, nret);
    put_words(outs, nouts);
    put_words(post, kWorldWords);
    printf(" %08x", g_log_count);
    put_words(g_log, g_log_len);
    printf("\n");
}
static FFrame& frame() {
    FFrame* f = (FFrame*)g_frame_store;
    f->Code = g_code;
    return *f;
}

static void case_player_can_see_me(uint32_t actor_id) {
    begin_record();
    DWORD seen = 0;
    ((AActor*)ptr_of(actor_id))->execPlayerCanSeeMe(frame(), &seen);
    const uint32_t args[] = {actor_id};
    const uint32_t ret[] = {seen};
    end_record("player_can_see_me", args, 1, ret, 1, NULL, 0);
}
static void case_test_can_see_me(uint32_t actor_id, uint32_t viewer_id) {
    begin_record();
    const INT seen = ((AActor*)ptr_of(actor_id))->TestCanSeeMe((APlayerPawn*)ptr_of(viewer_id));
    const uint32_t args[] = {actor_id, viewer_id};
    const uint32_t ret[] = {wrd(seen)};
    end_record("test_can_see_me", args, 2, ret, 1, NULL, 0);
}
static void case_pick(bool any, uint32_t pawn_id, FLOAT aim, FLOAT dist, FVector fire,
                      FVector start) {
    g_ai_best_aim = aim;
    g_ai_best_dist = dist;
    g_ai_fire_dir = fire;
    g_ai_proj_start = start;
    APawn* pawn = (APawn*)ptr_of(pawn_id);
    begin_record();
    uint32_t picked = 0;
    if (any) {
        AActor* result = NULL;
        pawn->execPickAnyTarget(frame(), &result);
        picked = object_id(result);
    } else {
        APawn* result = NULL;
        pawn->execPickTarget(frame(), &result);
        picked = object_id(result);
    }
    const uint32_t args[] = {pawn_id, bits(aim), bits(dist), bits(fire.X), bits(fire.Y),
                             bits(fire.Z), bits(start.X), bits(start.Y), bits(start.Z)};
    const uint32_t ret[] = {picked};
    const uint32_t outs[] = {bits(g_ai_best_aim), bits(g_ai_best_dist)};
    end_record(any ? "pick_any_target" : "pick_target", args, 9, ret, 1, outs, 2);
}
static void case_pawn_only(const char* name, uint32_t pawn_id, void (APawn::*fn)(FFrame&, void* const)) {
    begin_record();
    (((APawn*)ptr_of(pawn_id))->*fn)(frame(), NULL);
    const uint32_t args[] = {pawn_id};
    end_record(name, args, 1, NULL, 0, NULL, 0);
}
static void case_poll(uint32_t pawn_id, FLOAT delta) {
    begin_record();
    FLOAT result = delta;
    ((APawn*)ptr_of(pawn_id))->execPollWaitForLanding(frame(), &result);
    const uint32_t args[] = {pawn_id, bits(delta)};
    end_record("poll_wait_for_landing", args, 2, NULL, 0, NULL, 0);
}
static void case_find(uint32_t pawn_id, FLOAT delta) {
    g_ai_delta_time = delta;
    begin_record();
    DWORD stair = 0;
    ((APawn*)ptr_of(pawn_id))->execFindStairRotation(frame(), &stair);
    const uint32_t args[] = {pawn_id, bits(delta)};
    const uint32_t ret[] = {stair};
    end_record("find_stair_rotation", args, 2, ret, 1, NULL, 0);
}

static void random_pick_args(FLOAT& aim, FLOAT& dist, FVector& fire, FVector& start) {
    static const FLOAT kAim[] = {-1.f, 0.f, 0.3f, 0.9f, -0.5f};
    static const FLOAT kDist[] = {0.f, 50.f, 1000.f};
    static const FLOAT kScale[] = {1.f, 0.5f, 100.f};
    aim = kAim[rnd(5)];
    dist = kDist[rnd(3)];
    const FLOAT s = kScale[rnd(3)];
    fire = FVector(((FLOAT)rnd(3) - 1.f) * s, ((FLOAT)rnd(3) - 1.f) * s,
                   ((FLOAT)rnd(3) - 1.f) * s);
    start = FVector(rndf(-200, 200), rndf(-200, 200), rndf(0, 128));
}

int main() {
    static const uint32_t kPlayerCases = 24, kTestCases = 32, kPickCases = 48, kAddCases = 12,
                          kRemoveCases = 16, kStopCases = 12, kWaitCases = 12, kPollCases = 16,
                          kFindCases = 64;
    static const FLOAT kPollDelta[] = {0.f, 0.1f, 0.5f, 1.5f, 3.f};
    static const FLOAT kFindDelta[] = {0.1f, 0.25f, 0.33f, 0.34f, 0.5f, 1.0f};
    for (uint32_t c = 0; c < kPlayerCases; ++c) {
        build_world();
        case_player_can_see_me(1 + rnd(kPawns + kActors));
    }
    for (uint32_t c = 0; c < kTestCases; ++c) {
        build_world();
        const uint32_t actor = 1 + rnd(kPawns + kActors);
        case_test_can_see_me(actor, rnd(kPawns + 1));
    }
    for (uint32_t c = 0; c < kPickCases; ++c) {
        build_world();
        FLOAT aim, dist;
        FVector fire, start;
        random_pick_args(aim, dist, fire, start);
        case_pick(false, 1 + rnd(kPawns), aim, dist, fire, start);
    }
    for (uint32_t c = 0; c < kPickCases; ++c) {
        build_world();
        FLOAT aim, dist;
        FVector fire, start;
        random_pick_args(aim, dist, fire, start);
        case_pick(true, 1 + rnd(kPawns), aim, dist, fire, start);
    }
    for (uint32_t c = 0; c < kAddCases; ++c) {
        build_world();
        case_pawn_only("add_pawn", 1 + rnd(kPawns), &APawn::execAddPawn);
    }
    for (uint32_t c = 0; c < kRemoveCases; ++c) {
        build_world();
        case_pawn_only("remove_pawn", 1 + rnd(kPawns), &APawn::execRemovePawn);
    }
    for (uint32_t c = 0; c < kStopCases; ++c) {
        build_world();
        case_pawn_only("stop_waiting", 1 + rnd(kPawns), &APawn::execStopWaiting);
    }
    for (uint32_t c = 0; c < kWaitCases; ++c) {
        build_world();
        case_pawn_only("wait_for_landing", 1 + rnd(kPawns), &APawn::execWaitForLanding);
    }
    for (uint32_t c = 0; c < kPollCases; ++c) {
        build_world();
        case_poll(1 + rnd(kPawns), kPollDelta[rnd(5)]);
    }
    for (uint32_t c = 0; c < kFindCases; ++c) {
        build_world();
        case_find(1 + rnd(kPawns), kFindDelta[rnd(6)]);
    }
    return 0;
}
