// Original-object pawn-sense fixture harness. The function symbols are from the
// pinned UnPawn.cpp object; event dispatch and class probes below are explicit.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
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
FName ENGINE_HearNoise, ENGINE_EnemyNotVisible, ENGINE_Touch, ENGINE_SpecialHandling;

alignas(16) static char cls_actor[sizeof(UClass)], cls_pawn[sizeof(UClass)], cls_player[sizeof(UClass)], cls_inventory[sizeof(UClass)], cls_info[sizeof(UClass)], cls_zone[sizeof(UClass)], cls_level_info[sizeof(UClass)], cls_game[sizeof(UClass)], cls_pri[sizeof(UClass)];
UClass* AActor::PrivateStaticClass = (UClass*)cls_actor;
UClass* APawn::PrivateStaticClass = (UClass*)cls_pawn;
UClass* APlayerPawn::PrivateStaticClass = (UClass*)cls_player;
UClass* AInventory::PrivateStaticClass = (UClass*)cls_inventory;
UClass* AInfo::PrivateStaticClass = (UClass*)cls_info;
UClass* AZoneInfo::PrivateStaticClass = (UClass*)cls_zone;
UClass* ALevelInfo::PrivateStaticClass = (UClass*)cls_level_info;
UClass* AGameInfo::PrivateStaticClass = (UClass*)cls_game;
UClass* APlayerReplicationInfo::PrivateStaticClass = (UClass*)cls_pri;
UClass* AActor::GetPrivateStaticClassAActor(const TCHAR*) { return PrivateStaticClass; }
void AActor::InitializePrivateStaticClassAActor() {}
UClass* APawn::GetPrivateStaticClassAPawn(const TCHAR*) { return PrivateStaticClass; }
void APawn::InitializePrivateStaticClassAPawn() {}
UClass* APlayerPawn::GetPrivateStaticClassAPlayerPawn(const TCHAR*) { return PrivateStaticClass; }
void APlayerPawn::InitializePrivateStaticClassAPlayerPawn() {}
UClass* AInventory::GetPrivateStaticClassAInventory(const TCHAR*) { return PrivateStaticClass; }
void AInventory::InitializePrivateStaticClassAInventory() {}
static UClass* (* const g_static_class_exports[])(const TCHAR*) __attribute__((used)) = {
    &AActor::GetPrivateStaticClassAActor,
    &APawn::GetPrivateStaticClassAPawn,
    &APlayerPawn::GetPrivateStaticClassAPlayerPawn,
    &AInventory::GetPrivateStaticClassAInventory,
};
alignas(16) static char zone_default_storage[sizeof(AZoneInfo)];
static AZoneInfo* g_zone_default = (AZoneInfo*)zone_default_storage;
static APawn* g_pawn;
static APawn* g_other;
static ULevel* g_level;
static ALevelInfo* g_level_info;
static AZoneInfo* g_zone;
static AGameInfo* g_game;
static APlayerReplicationInfo* g_pri_pawn;
static APlayerReplicationInfo* g_pri_other;
static void* g_objects[10];
static AActor* g_special_sequence[8];
static uint32_t g_special_sequence_count, g_special_sequence_index;
static AActor* g_special_result;
static uint32_t object_id(const void* p) {
    if (!p) return 0;
    for (uint32_t i = 1; i < 10; ++i)
        if (g_objects[i] == p) return i;
    return 0xffffffffu;
}



// Distinct function identities ensure the event dispatcher cannot accidentally
// treat one UnrealScript event as another.
static int tag_hear, tag_invisible, tag_touch, tag_special;
static int32_t fname_word(FName n) { int32_t v; memcpy(&v, &n, sizeof v); return v; }
static FName g_hear, g_invisible, g_touch, g_special;
UFunction* UObject::FindFunctionChecked(FName name, UBOOL) {
    const int32_t id = fname_word(name);
    if (id == fname_word(g_hear)) return (UFunction*)&tag_hear;
    if (id == fname_word(g_invisible)) return (UFunction*)&tag_invisible;
    if (id == fname_word(g_touch)) return (UFunction*)&tag_touch;
    if (id == fname_word(g_special)) return (UFunction*)&tag_special;
    abort();
}

static void record_log(uint32_t, uint32_t, const uint32_t*, uint32_t, const uint32_t*);
// IsProbing is an out-of-line UObject method in this engine build. Keep it a
// concrete, name-sensitive script rather than allowing the generator to stub it.
static UBOOL g_probe_hear = 1;
static UBOOL g_probe_special = 1;
UBOOL UObject::IsProbing(FName name) {
    const int32_t id = fname_word(name);
    uint32_t kind;
    UBOOL result;
    if (id == fname_word(NAME_HearNoise)) {
        kind = 0;
        result = g_probe_hear;
    } else if (id == fname_word(NAME_SpecialHandling)) {
        kind = 1;
        result = g_probe_special;
    } else {
        abort();
    }
    const uint32_t args[] = {object_id(this), kind};
    const uint32_t outs[] = {(uint32_t)result};
    record_log(6, 2, args, 1, outs);
    return result;
}
// Original `UObject::IsValid` confirms the object-table identity.  The fixture
// harness owns a compact object table, so preserve that contract for every
// synthetic object it can expose to the original pawn code.
UBOOL UObject::IsValid() {
    return object_id(this) != 0 && object_id(this) != 0xffffffffu;
}

struct LogRecord {
    uint32_t kind, argc, args[16], outc, outs[8];
};
static LogRecord g_log[256];
static uint32_t g_nlog;
static void record_log(uint32_t kind, uint32_t argc, const uint32_t* args,
                       uint32_t outc, const uint32_t* outs) {
    if (g_nlog == 256 || argc > 16 || outc > 8) abort();
    LogRecord& r = g_log[g_nlog++];
    r.kind = kind; r.argc = argc; r.outc = outc;
    if (argc) memcpy(r.args, args, argc * sizeof(uint32_t));
    if (outc) memcpy(r.outs, outs, outc * sizeof(uint32_t));
}
static void emit_logs() {
    for (uint32_t i = 0; i < g_nlog; ++i) {
        const LogRecord& r = g_log[i];
        printf(" S %08x %08x", r.kind, r.argc);
        for (uint32_t j = 0; j < r.argc; ++j) printf(" %08x", r.args[j]);
        printf(" %08x", r.outc);
        for (uint32_t j = 0; j < r.outc; ++j) printf(" %08x", r.outs[j]);
    }
}
static UBOOL g_fast_line_result = 1;
static UBOOL g_fast_line_script[64];
static uint32_t g_fast_line_count, g_fast_line_index;
static FLOAT g_frand_result = 0.75f;
static FLOAT g_frand_script[64];
static uint32_t g_frand_count, g_frand_index;
static FLOAT g_trace_time = 1.f;
static FVector g_trace_normal(0.f, 0.f, 1.f);
static FLOAT g_trace_times[64];
static FVector g_trace_normals[64];
static uint32_t g_trace_count, g_trace_index;
static FLOAT next_trace_time() {
    const uint32_t i = g_trace_index++;
    return i < g_trace_count ? g_trace_times[i] : g_trace_time;
}
static FVector next_trace_normal(uint32_t i) {
    return i < g_trace_count ? g_trace_normals[i] : g_trace_normal;
}
static uint32_t bits(float f);
static void print_vec(const FVector& v) {
    printf("%08x %08x %08x", bits(v.X), bits(v.Y), bits(v.Z));
}
FLOAT appFrand() {
    const uint32_t i = g_frand_index++;
    const FLOAT value = i < g_frand_count ? g_frand_script[i] : g_frand_result;
    const uint32_t outs[] = {bits(value)};
    record_log(3, 0, NULL, 1, outs);
    return value;
}
void AActor::setPhysics(BYTE phys, AActor*) {
    Physics = phys;
    const uint32_t args[] = {object_id(this), (uint32_t)phys};
    record_log(4, 2, args, 0, NULL);
}
FVector AActor::GetCylinderExtent() const {
    FVector out(CollisionRadius, CollisionRadius, CollisionHeight);
    const uint32_t args[] = {object_id(this)};
    const uint32_t outs[] = {bits(out.X), bits(out.Y), bits(out.Z)};
    record_log(5, 1, args, 3, outs);
    return out;
}
BYTE UModel::FastLineCheck(FVector end, FVector start) {
    const uint32_t i = g_fast_line_index++;
    const UBOOL value = i < g_fast_line_count ? g_fast_line_script[i] : g_fast_line_result;
    const uint32_t args[] = {bits(end.X), bits(end.Y), bits(end.Z),
        bits(start.X), bits(start.Y), bits(start.Z)};
    const uint32_t outs[] = {(uint32_t)value};
    record_log(1, 6, args, 1, outs);
    return (BYTE)value;
}
static UBOOL trace_impl(FCheckResult& hit,AActor* source,const FVector& end,const FVector& start,DWORD trace,FVector extent) {
    const uint32_t i=g_trace_index;const FLOAT time=next_trace_time();const FVector normal=next_trace_normal(i);
    const uint32_t args[]={object_id(source),bits(end.X),bits(end.Y),bits(end.Z),bits(start.X),bits(start.Y),bits(start.Z),trace,bits(extent.X),bits(extent.Y),bits(extent.Z)};
    const uint32_t outs[]={bits(time),bits(normal.X),bits(normal.Y),bits(normal.Z)};record_log(2,11,args,4,outs);hit.Time=time;hit.Normal=normal;return time==1.f;
}
UBOOL ULevel::SingleLineCheck(FCheckResult& hit,AActor* source,const FVector& end,const FVector& start,DWORD trace,FVector extent,BYTE) {
    return trace_impl(hit,source,end,start,trace,extent);
}
static UBOOL level_trace_hook(ULevel*,FCheckResult& hit,AActor* source,const FVector& end,const FVector& start,DWORD trace,FVector extent,BYTE) {
    return trace_impl(hit,source,end,start,trace,extent);
}
static void dispatch(AActor* self, UFunction* fn, void* parms, void* result) {
    (void)result;
    if (fn == (UFunction*)&tag_hear) {
        APawn_eventHearNoise_Parms* p = (APawn_eventHearNoise_Parms*)parms;
        uint32_t loudness; memcpy(&loudness, &p->Loudness, sizeof loudness);
        const uint32_t args[] = {object_id(self), loudness, object_id(p->NoiseMaker)};
        record_log(23, 3, args, 0, NULL);
    } else if (fn == (UFunction*)&tag_invisible) {
        const uint32_t args[] = {object_id(self)};
        record_log(24, 1, args, 0, NULL);
    } else if (fn == (UFunction*)&tag_touch) {
        AActor_eventTouch_Parms* p = (AActor_eventTouch_Parms*)parms;
        const uint32_t args[] = {object_id(self), object_id(p->Other)};
        record_log(25, 2, args, 0, NULL);
    } else if (fn == (UFunction*)&tag_special) {
        AActor_eventSpecialHandling_Parms* p = (AActor_eventSpecialHandling_Parms*)parms;
        p->ReturnValue = g_special_sequence_index < g_special_sequence_count
            ? g_special_sequence[g_special_sequence_index++] : g_special_result;
        const uint32_t args[] = {object_id(self), object_id(p->Other)};
        const uint32_t outs[] = {object_id(p->ReturnValue)};
        record_log(26, 2, args, 1, outs);
    } else abort();
}
void AActor::ProcessEvent(UFunction* fn, void* parms, void* result) { dispatch(this, fn, parms, result); }
static void process_event_hook(AActor* self, UFunction* fn, void* parms, void* result) { dispatch(self, fn, parms, result); }
static INT g_reachable_result = 1;
static INT g_path_success = 1;
static AActor* g_path_result;
INT PawnSenseActorReachableHook(APawn* pawn, AActor* other, INT know_visible) {
    const uint32_t args[] = {object_id(pawn), object_id(other), (uint32_t)know_visible};
    const uint32_t outs[] = {(uint32_t)g_reachable_result};
    record_log(20, 3, args, 1, outs);
    return g_reachable_result;
}
INT PawnSenseFindPathTowardHook(APawn* pawn, AActor* goal, INT single,
                                AActor*& best_path, INT clear) {
    const uint32_t args[] = {object_id(pawn), object_id(goal), (uint32_t)single,
        object_id(best_path), (uint32_t)clear};
    best_path = g_path_result;
    const uint32_t outs[] = {(uint32_t)g_path_success, object_id(best_path)};
    record_log(21, 5, args, 2, outs);
    return g_path_success;
}

static uint32_t bits(float f) { uint32_t u; memcpy(&u, &f, 4); return u; }
FVector PawnSenseSafeNormalHook(FVector v) {
    FVector out = v.SafeNormal();
    const uint32_t args[] = {bits(v.X), bits(v.Y), bits(v.Z)};
    const uint32_t outs[] = {bits(out.X), bits(out.Y), bits(out.Z)};
    record_log(10, 3, args, 3, outs);
    return out;
}
FRotator PawnSenseVectorRotationHook(FVector v) {
    FRotator out = v.Rotation();
    const uint32_t args[] = {bits(v.X), bits(v.Y), bits(v.Z)};
    const uint32_t outs[] = {(uint32_t)out.Pitch, (uint32_t)out.Yaw, (uint32_t)out.Roll};
    record_log(11, 3, args, 3, outs);
    return out;
}
UObject* PawnSenseGetZoneDefaultObjectHook(UClass* cls) {
    if (cls != AZoneInfo::PrivateStaticClass) abort();
    return (UObject*)g_zone_default;
}
static size_t vt_slot(const void* member) {
    uintptr_t word[2];
    memcpy(word, member, sizeof word);
    if (!(word[1] & 1)) abort();
    return word[0] / sizeof(void*);
}
static void* actor_vtable[4096];
static uint32_t g_input[78], g_args[5], g_argc;
static AActor* g_best_path;
static UModel* g_model;
static AActor* g_level_actors[1];
static void* level_vtable[4096];
static INT g_last_return;
static FLOAT g_last_float;
static void print_word(uint32_t w) { printf(" %08x",w); }
static uint32_t word(float x) { return bits(x); }
static FLOAT input_float(uint32_t i) { FLOAT x; memcpy(&x,&g_input[i],4); return x; }
static FVector input_vec(uint32_t i) { return FVector(input_float(i),input_float(i+1),input_float(i+2)); }
static void set_float(uint32_t i,FLOAT x) { g_input[i]=bits(x); }
static void set_vec(uint32_t i,FLOAT x,FLOAT y,FLOAT z) { set_float(i,x);set_float(i+1,y);set_float(i+2,z); }
static void install_level_trace() {
    auto m=&ULevel::SingleLineCheck;
    level_vtable[vt_slot(&m)]=(void*)&level_trace_hook;
    void** vt=level_vtable;memcpy(g_level,&vt,sizeof vt);
}
static void install_actor_events(void* actor) {
    auto m=&AActor::ProcessEvent;
    actor_vtable[vt_slot(&m)]=(void*)&process_event_hook;
    void** vt=actor_vtable;memcpy(actor,&vt,sizeof vt);
}
static void configure_case(uint32_t fn,uint32_t c) {
    memset(g_input,0,sizeof g_input);memset(g_args,0,sizeof g_args);g_argc=0;
    g_input[0]=1;set_vec(2,0,0,0);set_vec(5,-1,0,0);set_vec(8,0,0,0);set_vec(11,0,0,0);
    set_float(20,16);set_float(21,40);set_float(22,24);set_float(23,100);set_float(24,30);
    set_float(25,.5f);set_float(26,.5f);set_float(27,320);set_float(28,320);set_float(29,320);
    set_float(30,-1);set_float(31,1000);set_float(32,-1);set_float(33,1);set_float(34,1);set_float(35,0);set_float(36,0);set_float(37,-1);
    g_input[38]=2;g_input[39]=2;g_input[40]=1;g_input[41]=4;g_input[42]=8;
    set_vec(43,100,0,0);set_float(46,0);set_float(47,1);set_vec(48,200,0,0);set_float(51,0);set_float(52,.5f);
    set_vec(53,10,0,0);set_float(56,16);set_float(57,40);g_input[59]=1;g_input[60]=9;g_input[64]=1;g_input[65]=1;g_input[68]=0;set_float(69,10);g_input[70]=0;g_input[71]=1;set_float(72,-1);set_vec(73,100,0,0);set_float(76,300);set_float(77,1);
    g_fast_line_result=1;g_frand_result=.75f;g_trace_time=1;g_trace_normal=FVector(0,0,1);g_trace_count=0;g_fast_line_count=0;g_frand_count=0;g_probe_hear=1;g_probe_special=1;g_reachable_result=1;g_path_success=1;g_path_result=g_other;g_special_result=g_other;g_special_sequence_count=0;g_special_sequence_index=0;
    if(fn==0) {g_input[0]=(uint32_t[]){1,2,4,3,10,0,0}[c];g_argc=1;g_args[0]=bits((FLOAT[]){20,50,100,30,40,25,40}[c]);}
    if(fn==1) {g_argc=3;g_args[0]=bits(100);g_args[1]=bits(0);g_args[2]=bits(0);
        if(c<3){g_input[0]=2;set_float(37,c==0?100:1);set_float(72,1);set_vec(8,0,0,-10);set_vec(73,100,0,50);}
        if(c==3)g_input[65]=3;
        if(c==4||c==5){set_vec(2,10,0,0);set_vec(5,0,0,0);set_vec(8,10,0,0);set_vec(73,0,0,0);if(c==5)set_vec(73,-100,0,0);}
        if(c==6)set_vec(73,2,0,0);if(c==7)set_vec(73,0,0,100);if(c==8)set_float(30,0);
        if(c==9||c==10){g_input[39]=2;set_vec(73,c==9?1:100,0,0);}
        if(c==11||c==15){g_input[0]=4;g_input[14]=0;g_input[15]=16384;}
        if(c==12)set_vec(8,100,0,0);if(c==13||c==14)g_input[1]|=4;
    }
    if(fn==2){g_argc=3;g_args[0]=bits(100);g_args[1]=bits(10);g_args[2]=0;if(c==0)g_input[0]=10;if(c==1)g_args[0]=bits(1);if(c==2)g_args[1]=bits(65530);if(c==3)g_args[1]=bits(10000);if(c==4){g_input[0]=1;g_input[65]=1;}}
    if(fn==3){g_argc=2;g_args[0]=c==0?0:2;g_args[1]=(c==4||c==5);if(c==1||c==2)g_input[38]=2;if(c==3)set_vec(53,2000,0,0);if(c==4)g_input[65]=2;if(c==5)g_input[65]=0;if(c==6)set_float(32,1);if(c==7)set_vec(53,1000,0,0);if(c==8||c==9)set_vec(53,1,0,0);if(c==10)set_vec(53,600,0,0);if(c==11)g_input[1]|=8;if(c==12)g_frand_result=.05f;if(c>=13)set_vec(53,200,0,0);}
    if(fn==4){g_argc=5;g_args[0]=bits(100);g_args[1]=0;g_args[2]=0;g_args[3]=bits(1);g_args[4]=2;if(c==0)set_vec(53,1000,0,0);if(c==1)set_vec(43,0,0,0);if(c==2)set_float(36,2);if(c==3)g_fast_line_result=0;if(c==5){g_input[68]=1;g_input[70]=g_input[71]=1;set_vec(53,1000,0,0);}if(c==6||c==7){g_input[68]=1;g_input[70]=g_input[71]=1;g_input[58]=1;g_fast_line_result=c==7;}}
    if(fn==5){g_argc=2;g_args[0]=bits(1);g_args[1]=1;if(c==0)g_input[40]=0;if(c==1||c==2){set_float(46,0);set_float(51,0);}if(c==3||c==4){set_float(46,-100);set_float(51,-100);}if(c==5){set_float(46,10);set_float(51,0);}if(c==6){set_float(46,10);set_float(51,0);set_float(47,.9f);set_float(52,.8f);g_input[70]=1;}if(c==7)set_float(36,0);if(c==8)g_input[65]=0;if(c==9||c==10){g_input[40]=2;g_input[65]=2;g_input[68]=1;g_input[70]=1;}}
    if(fn==6){if(c==0)g_input[38]=0;if(c==1)g_fast_line_result=0;if(c==2){g_fast_line_result=1;set_float(69,42);}}
    if(fn==7){g_input[0]=c==0?2:1;if(c==1)set_vec(2,0,0,1);if(c==3){g_trace_times[0]=.5f;g_trace_count=1;}if(c==4||c==5||c==6){g_input[0]=c==4?3:4;g_trace_times[0]=c==4?.5f:1;g_trace_count=1;}if(c==7||c==8||c==9||c==10){g_trace_count=2;g_trace_times[0]=c==8||c==9?1:.5f;g_trace_times[1]=.5f;}if(c==11){g_input[1]|=16;g_trace_times[0]=.5f;g_trace_count=1;}if(c==12)g_input[1]|=1;if(c==13)g_input[1]&=~16u;if(c>=14){g_input[0]=4;g_trace_times[0]=1;g_trace_count=1;}}
    if(fn==8){g_argc=3;g_args[0]=0;g_args[1]=bits(1);g_args[2]=bits(.1f);if(c==0)g_input[1]|=1;if(c==2)g_input[65]=2;if(c==3)g_input[62]=1;if(c==4)g_input[63]=1;if(c==5)g_input[59]=2;}
    if(fn==9){g_argc=1;g_args[0]=2;g_input[1]|=32;if(c==0)g_special_result=NULL;if(c==2)g_input[1]&=~32u;if(c==3){g_special_result=g_other;g_probe_special=0;}if(c>=4&&c<=6){g_special_sequence_count=2;g_special_sequence[0]=g_other;g_special_sequence[1]=c==4?NULL:c==5?g_other:g_pawn;}if(c>=7){g_special_result=NULL;g_path_result=g_other;g_path_success=c>=9;}}
}
static UClass* class_for(uint32_t c) {
    return c==1?APawn::PrivateStaticClass:c==2?APlayerPawn::PrivateStaticClass:c==3?AInventory::PrivateStaticClass:AActor::PrivateStaticClass;
}
static void run_case(uint32_t fn,uint32_t c) {
    configure_case(fn,c);APawn*p=g_pawn;memset(p,0,sizeof(APawn));memset(g_other,0,sizeof(APawn));
    p->Class=class_for(g_input[64]);g_other->Class=class_for(g_input[65]);
    g_objects[1]=p;g_objects[2]=g_other;g_objects[3]=g_zone;g_objects[4]=g_level;g_objects[5]=g_objects[6]=g_objects[7]=g_other;g_objects[8]=g_pri_pawn;g_objects[9]=g_pri_other;g_best_path=(AActor*)g_objects[5];
    p->Physics=(BYTE)g_input[0];p->Location=input_vec(2);p->OldLocation=input_vec(5);p->Velocity=input_vec(8);p->Acceleration=input_vec(11);p->Rotation=FRotator((INT)g_input[14],(INT)g_input[15],(INT)g_input[16]);p->DesiredRotation=FRotator((INT)g_input[17],(INT)g_input[18],(INT)g_input[19]);
    p->CollisionRadius=input_float(20);p->CollisionHeight=input_float(21);p->BaseEyeHeight=input_float(22);p->AccelRate=input_float(23);p->MeleeRange=input_float(24);p->AvgPhysicsTime=input_float(25);p->DesiredSpeed=input_float(26);p->GroundSpeed=input_float(27);p->AirSpeed=input_float(28);p->WaterSpeed=input_float(29);p->MoveTimer=input_float(30);p->SightRadius=input_float(31);p->PeripheralVision=input_float(32);p->Skill=input_float(33);p->HearingThreshold=input_float(34);p->Alertness=input_float(35);p->Stimulus=input_float(36);p->Enemy=(APawn*)g_objects[g_input[38]];p->MoveTarget=(AActor*)g_objects[g_input[39]];p->Instigator=(APawn*)g_objects[g_input[40]];p->XLevel=g_level;p->Level=g_level_info;p->PlayerReplicationInfo=g_pri_pawn;p->Region.Zone=g_zone;p->Destination=input_vec(73);p->JumpZ=input_float(76);p->NetPriority=input_float(77);
    p->bIsPlayer=(g_input[1]&1)!=0;p->bCanStrafe=(g_input[1]&2)!=0;p->bReducedSpeed=(g_input[1]&4)!=0;p->bLOSflag=(g_input[1]&8)!=0;p->bCanJump=(g_input[1]&16)!=0;p->bCanDoSpecial=(g_input[1]&32)!=0;p->bHidden=(g_input[1]&64)!=0;p->bJumpOffPawn=(g_input[1]&128)!=0;
    p->noise1spot=input_vec(43);p->noise1time=input_float(46);p->noise1loudness=input_float(47);p->noise2spot=input_vec(48);p->noise2time=input_float(51);p->noise2loudness=input_float(52);p->Weapon=(AWeapon*)g_objects[g_input[61]];
    g_other->Location=input_vec(53);g_other->CollisionRadius=input_float(56);g_other->CollisionHeight=input_float(57);g_other->Visibility=(BYTE)g_input[58];g_other->Physics=(BYTE)g_input[59];g_other->Instigator=(APawn*)g_objects[g_input[40]];g_other->XLevel=g_level;g_other->Level=g_level_info;g_other->PlayerReplicationInfo=g_pri_other;g_other->bHidden=(g_input[62]&1)!=0;g_other->bNetOwner=(g_input[62]&2)!=0;
    ((APawn*)g_other)->bIsPlayer=(g_input[62]&4)!=0;((APawn*)g_other)->Weapon=(AWeapon*)g_objects[g_input[63]];((APawn*)g_other)->nextPawn=(APawn*)g_objects[g_input[67]];
    g_zone->Class=AZoneInfo::PrivateStaticClass;g_level_info->Class=ALevelInfo::PrivateStaticClass;g_game->Class=AGameInfo::PrivateStaticClass;g_pri_pawn->Class=APlayerReplicationInfo::PrivateStaticClass;g_pri_other->Class=APlayerReplicationInfo::PrivateStaticClass;
    g_zone->ZoneGravity=FVector(0,0,input_float(37));g_zone_default->ZoneGravity=FVector(0,0,input_float(72));g_level_info->PawnList=p;g_level_info->TimeSeconds=input_float(69);g_level_info->Game=g_game;g_game->bTeamGame=g_input[68]!=0;g_pri_pawn->Team=(BYTE)g_input[70];g_pri_other->Team=(BYTE)g_input[71];p->nextPawn=(APawn*)g_objects[g_input[66]];
    g_level->TimeSeconds=input_float(69);g_level->Model=g_model;g_level_actors[0]=g_level_info;g_level->Actors.Data=g_level_actors;g_level->Actors.ArrayNum=1;g_level->Actors.ArrayMax=1;
    g_nlog=g_trace_index=g_frand_index=g_fast_line_index=g_special_sequence_index=0;install_actor_events(p);install_actor_events(g_other);install_level_trace();
    g_last_return=0;g_last_float=0;
    if(fn==0)p->setMoveTimer(*(FLOAT*)&g_args[0]);
    else if(fn==1)g_last_return=p->moveToward(FVector(*(FLOAT*)&g_args[0],*(FLOAT*)&g_args[1],*(FLOAT*)&g_args[2]));
    else if(fn==2)g_last_return=p->rotateToward(FVector(*(FLOAT*)&g_args[0],*(FLOAT*)&g_args[1],*(FLOAT*)&g_args[2]));
    else if(fn==3)g_last_return=p->LineOfSightTo((AActor*)g_objects[g_args[0]],g_args[1]);
    else if(fn==4)g_last_return=p->CanHear(FVector(*(FLOAT*)&g_args[0],*(FLOAT*)&g_args[1],*(FLOAT*)&g_args[2]),*(FLOAT*)&g_args[3],(APawn*)g_objects[g_args[4]]);
    else if(fn==5)((AActor*)g_objects[g_args[1]])->CheckNoiseHearing(*(FLOAT*)&g_args[0]);
    else if(fn==6)p->CheckEnemyVisible();
    else if(fn==7)g_last_return=p->PickWallAdjust();
    else if(fn==8)g_last_float=p->APawn::GetNetPriority((AActor*)g_objects[g_args[0]],*(FLOAT*)&g_args[1],*(FLOAT*)&g_args[2]);
    else {g_best_path=(AActor*)g_objects[g_args[0]];p->HandleSpecial(g_best_path);}
}
static void emit_case(uint32_t fn,uint32_t c) {
    run_case(fn,c);uint32_t n=78+g_argc;printf("%08x %08x %08x %08x",0x43,fn,c,n);for(unsigned i=0;i<78;i++)print_word(g_input[i]);for(unsigned i=0;i<g_argc;i++)print_word(g_args[i]);
    uint32_t ret=fn==8?bits(g_last_float):fn==0||fn==5||fn==6||fn==9?0:(uint32_t)g_last_return;print_word(ret);uint32_t s[42]={};unsigned q=0;
#define SV(v) do{uint32_t w[3]={bits((v).X),bits((v).Y),bits((v).Z)};memcpy(s+q,w,12);q+=3;}while(0)
    SV(g_pawn->Location);SV(g_pawn->Velocity);SV(g_pawn->Acceleration);s[q++]=g_pawn->Rotation.Pitch;s[q++]=g_pawn->Rotation.Yaw;s[q++]=g_pawn->Rotation.Roll;s[q++]=g_pawn->DesiredRotation.Pitch;s[q++]=g_pawn->DesiredRotation.Yaw;s[q++]=g_pawn->DesiredRotation.Roll;s[q++]=bits(g_pawn->MoveTimer);s[q++]=bits(g_pawn->DesiredSpeed);s[q++]=bits(g_pawn->Stimulus);s[q++]=bits(g_pawn->LastSeenTime);SV(g_pawn->LastSeenPos);SV(g_pawn->LastSeeingPos);SV(g_pawn->Destination);s[q++]=g_pawn->Physics;s[q++]=g_pawn->bIsPlayer|(g_pawn->bCanStrafe<<1)|(g_pawn->bReducedSpeed<<2)|(g_pawn->bLOSflag<<3)|(g_pawn->bCanJump<<4)|(g_pawn->bCanDoSpecial<<5)|(g_pawn->bHidden<<6)|(g_pawn->bJumpOffPawn<<7);SV(g_pawn->noise1spot);s[q++]=bits(g_pawn->noise1time);s[q++]=bits(g_pawn->noise1loudness);SV(g_pawn->noise2spot);s[q++]=bits(g_pawn->noise2time);s[q++]=bits(g_pawn->noise2loudness);s[q++]=object_id(g_pawn->SpecialGoal);s[q++]=object_id(g_best_path);
#undef SV
    print_word(42);for(unsigned i=0;i<42;i++)print_word(s[i]);print_word(g_nlog);emit_logs();putchar('\n');
}
int main() {
    const int32_t ids[4]={0x5101,0x5102,0x5103,0x5104};memcpy(&ENGINE_HearNoise,ids,4);memcpy(&ENGINE_EnemyNotVisible,ids+1,4);memcpy(&ENGINE_Touch,ids+2,4);memcpy(&ENGINE_SpecialHandling,ids+3,4);g_hear=ENGINE_HearNoise;g_invisible=ENGINE_EnemyNotVisible;g_touch=ENGINE_Touch;g_special=ENGINE_SpecialHandling;
    AActor::PrivateStaticClass->SuperField=NULL;APawn::PrivateStaticClass->SuperField=AActor::PrivateStaticClass;APlayerPawn::PrivateStaticClass->SuperField=APawn::PrivateStaticClass;AInventory::PrivateStaticClass->SuperField=AActor::PrivateStaticClass;AInfo::PrivateStaticClass->SuperField=AActor::PrivateStaticClass;AZoneInfo::PrivateStaticClass->SuperField=AInfo::PrivateStaticClass;ALevelInfo::PrivateStaticClass->SuperField=AZoneInfo::PrivateStaticClass;AGameInfo::PrivateStaticClass->SuperField=AInfo::PrivateStaticClass;APlayerReplicationInfo::PrivateStaticClass->SuperField=AInfo::PrivateStaticClass;
    g_pawn=(APawn*)calloc(1,sizeof(APawn));g_other=(APawn*)calloc(1,sizeof(APawn));g_level=(ULevel*)calloc(1,sizeof(ULevel));g_model=(UModel*)calloc(1,sizeof(UModel));g_level_info=(ALevelInfo*)calloc(1,sizeof(ALevelInfo));g_zone=(AZoneInfo*)calloc(1,sizeof(AZoneInfo));g_game=(AGameInfo*)calloc(1,sizeof(AGameInfo));g_pri_pawn=(APlayerReplicationInfo*)calloc(1,sizeof(APlayerReplicationInfo));g_pri_other=(APlayerReplicationInfo*)calloc(1,sizeof(APlayerReplicationInfo));
    if(!g_pawn||!g_other||!g_level||!g_model||!g_level_info||!g_zone||!g_game||!g_pri_pawn||!g_pri_other)return 2;
    g_zone_default->Class=AZoneInfo::PrivateStaticClass;AZoneInfo::PrivateStaticClass->Defaults.Data=(BYTE*)g_zone_default;AZoneInfo::PrivateStaticClass->Defaults.ArrayNum=sizeof(AZoneInfo);AZoneInfo::PrivateStaticClass->Defaults.ArrayMax=sizeof(AZoneInfo);AZoneInfo::PrivateStaticClass->PropertiesSize=sizeof(AZoneInfo);
    g_level->Model=g_model;g_level_actors[0]=g_level_info;g_level->Actors.Data=g_level_actors;g_level->Actors.ArrayNum=1;g_level->Actors.ArrayMax=1;
    g_objects[3]=g_zone;g_objects[4]=g_level;g_objects[8]=g_pri_pawn;g_objects[9]=g_pri_other;const int counts[10]={7,16,6,16,8,11,3,17,7,11};for(int f=0;f<10;f++)for(int c=0;c<counts[f];c++)emit_case(f,c);return 0;
}
