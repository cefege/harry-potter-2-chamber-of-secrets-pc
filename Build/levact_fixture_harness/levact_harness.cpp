// Fixture harness for the level-actor helpers in UnLevAct.cpp.
// Compiled against the ORIGINAL UnLevAct.cpp (git snapshot) by gen_levact_fixtures.sh.
// Prints one record per call. Rotation records start with the op name (yaw/roll/pitch/full);
// check records start with "chk". Values are IEEE bit patterns (8 hex digits) so the Rust replay
// compares bits exactly.
#include "EnginePrivate.h"
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <cstdlib>
#include <cstdarg>

FVector FindDeltaFromRotationYaw(FVector origin, FVector point, int angle);
FVector FindDeltaFromRotationRoll(FVector origin, FVector point, int angle);
FVector FindDeltaFromRotationPitch(FVector origin, FVector point, int angle);
FVector FindDeltaFromRotation(FVector origin, FVector point, int yaw, int roll, int pitch);
UBOOL DoMultiLineCheck(AActor* Actor);

// Libm wrappers with the same signatures as Core/Src/UnAnsi.cpp (mangled names match).
DOUBLE appSin(DOUBLE v) { return sin(v); }
DOUBLE appCos(DOUBLE v) { return cos(v); }
DOUBLE appSqrt(DOUBLE v) { return sqrt(v); }
// Same body as Core/Src/UnAnsi.cpp appQsort.
void appQsort( void* Base, INT Num, INT Width, int(CDECL *Compare)(const void* A, const void* B ) )
{
	qsort( Base, Num, Width, Compare );
}

static uint64_t g_state = 0x9E3779B97F4A7C15ull;
static uint32_t next_u32() {
	g_state = g_state * 6364136223846793005ull + 1442695040888963407ull;
	return (uint32_t)(g_state >> 33);
}
static float rand_coord() {
	// Mix of small/large magnitudes, exact zeros and negatives.
	uint32_t r = next_u32();
	switch (r % 8) {
		case 0: return 0.0f;
		case 1: return -((float)(next_u32() % 2048)) / 8.0f;
		case 2: return (float)(next_u32() % 4096) * 0.37f - 900.0f;
		case 3: return (float)(int32_t)next_u32() * (1.0f / 65536.0f);
		default: return ((float)(next_u32() % 200000) / 100.0f) - 1000.0f;
	}
}
static int rand_angle() {
	uint32_t r = next_u32();
	switch (r % 6) {
		case 0: return 0;
		case 1: return 16384;
		case 2: return -16384;
		case 3: return 32768;
		case 4: return (int)(next_u32() % 65536) - 32768;
		default: return (int)(next_u32() % 131072) - 65536;
	}
}
static uint32_t bits(float f) { uint32_t u; memcpy(&u, &f, 4); return u; }
static void emit(const char* op, FVector a, FVector b, int i0, int i1, int i2, FVector out) {
	printf("%s %08x %08x %08x %08x %08x %08x %08x %08x %08x -> %08x %08x %08x\n", op,
		bits(a.X), bits(a.Y), bits(a.Z), bits(b.X), bits(b.Y), bits(b.Z),
		(uint32_t)i0, (uint32_t)i1, (uint32_t)i2, bits(out.X), bits(out.Y), bits(out.Z));
}

// ---- check-function world ---------------------------------------------------------
// Pool actors: 0..2 plain AActor, 3 AMover, 4 AZoneInfo, 5 ALevelInfo (Actors(0)).
// Scripted services are pure functions of their arguments (hmix over exact bit patterns).
// Hit fields are exact small values (k/1024 time, integer/4 location, {0,1} normal), so the
// scripted side has no float arithmetic that a compiler could contract.

static uint64_t hmix(uint64_t h, uint64_t w) {
	uint64_t z = h ^ (w + 0x9E3779B97F4A7C15ULL + (h << 6) + (h >> 2));
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
	return z ^ (z >> 31);
}
static uint64_t hvec(uint64_t h, FVector v) { return hmix(hmix(hmix(h, bits(v.X)), bits(v.Y)), bits(v.Z)); }
static uint64_t draw(uint64_t h, uint64_t k) { return hmix(h, 0x1000 + k); }
static float coord5(uint64_t u) { return (float)((int)(u % 5) - 2); }
static float bit01(uint64_t u) { return (float)(u % 2); }

// Fill a hit's scripted fields (not Actor/Next/Primitive).
static void fill_hit(FCheckResult& R, uint64_t h) {
	R.Time = (float)(draw(h, 1) % 1024) / 1024.0f;
	R.Location = FVector(coord5(draw(h, 2)), coord5(draw(h, 3)), coord5(draw(h, 4)));
	R.Normal = FVector(bit01(draw(h, 5)), bit01(draw(h, 6)), bit01(draw(h, 7)));
	R.Item = (INT)(draw(h, 8) % 1000) - 500;
}

static AActor* g_pool[6];
static ULevel* g_lvl;
static UModel* g_model;
static FCollisionHashBase* g_hash;
static AActor* g_actors[6];
static ALevelInfo* g_levinfo;
static char g_membuf[1 << 20] __attribute__((aligned(16)));
static char g_cls_raw[8][sizeof(UClass)] __attribute__((aligned(16)));
static char g_obj_raw[7][8192] __attribute__((aligned(16)));
static char g_primitive_raw[2048] __attribute__((aligned(16)));
static UPrimitive* g_test_primitive;
static void* g_actor_vt[512];
static void* g_primitive_vt[512];
static bool g_force_self_hit;
static bool g_actor_ops;
static bool g_move_center_mode;
static bool g_empty_encroachment;
static FVector g_trace_start;
static FVector g_trace_end;
static int g_trace_calls;
UBOOL GIsEditor = 0;
FActorMovementObserver* GActorMovementObserver = NULL;

extern "C" UBOOL hp2_rs_engine_levact_do_multi_line_check( UBOOL IsMover, INT EncroachType )
{
	// Match the Rust dispatch policy: non-movers trace, movers trace for encroach types 0-3.
	return IsMover ? EncroachType < 4 : 1;
}

extern "C" FCheckResult* hp2_rs_engine_levact_multi_line_check(
	const void* Services, void* GMemPtr, void* HashPtr, void* MemPtr, void* LevelPtr,
	const FVector* End, const FVector* Start, const FVector* Extent, INT CheckActors, BYTE Extra )
{
	(void)Services;
	(void)GMemPtr;
	FCollisionHashBase* Hash = (FCollisionHashBase*)HashPtr;
	FMemStack& Mem = *(FMemStack*)MemPtr;
	if( LevelPtr )
	{
		FCheckResult WorldHit(1.f);
		if( !g_model->LineCheck( WorldHit, NULL, *End, *Start, *Extent, Extra ) )
		{
			FCheckResult* Node = new(Mem) FCheckResult(WorldHit);
			Node->Actor = (AActor*)LevelPtr;
			Node->GetNext() = NULL;
			return Node;
		}
	}
	if( CheckActors || LevelPtr )
		return Hash->ActorLineCheck( Mem, *End, *Start, *Extent, Extra );
	return NULL;
}

extern "C" FCheckResult* hp2_rs_engine_levact_multi_point_check(
	const void* Services, void* GMemPtr, void* HashPtr, void* MemPtr, void* LevelPtr,
	const FVector* Location, const FVector* Extent, DWORD Flags, INT CheckActors )
{
	(void)Services;
	FMemStack& Mem = *(FMemStack*)MemPtr;
	FCheckResult* Actors = NULL;
	if( CheckActors && HashPtr )
		Actors = ((FCollisionHashBase*)HashPtr)->ActorPointCheck(Mem, *Location, *Extent, Flags);
	if( LevelPtr )
	{
		FCheckResult Hit(1.f);
		if( !g_model->PointCheck(Hit, NULL, *Location, *Extent, Flags) )
		{
			FCheckResult* Node = new(*(FMemStack*)GMemPtr) FCheckResult(Hit);
			Node->Actor = (AActor*)LevelPtr;
			Node->GetNext() = Actors;
			return Node;
		}
	}
	return Actors;
}

extern "C" INT hp2_rs_engine_levact_single_point_check(
	const void* Services, void* GMemPtr, void* HashPtr, FCheckResult* Hit, void* LevelPtr,
	const FVector* Location, const FVector* Extent, DWORD Flags, INT CheckActors )
{
	(void)Services;
	if( LevelPtr && !g_model->PointCheck( *Hit, NULL, *Location, *Extent, Flags ) )
		return 0;
	if( CheckActors && HashPtr )
	{
		FCheckResult* ActorHit = ((FCollisionHashBase*)HashPtr)->ActorPointCheck(
			*(FMemStack*)GMemPtr, *Location, *Extent, Flags );
		if( ActorHit )
		{
			*Hit = *ActorHit;
			return 0;
		}
	}
	return 1;
}

extern "C" INT hp2_rs_engine_levact_single_line_check(
	const void* Services, void* GMemPtr, void* HashPtr, FCheckResult* Hit, void* Source,
	void* LevelPtr, const FVector* End, const FVector* Start, const FVector* Extent,
	DWORD TraceFlags, BYTE Extra )
{
	FCheckResult* Check = hp2_rs_engine_levact_multi_line_check(
		Services, GMemPtr, HashPtr, GMemPtr, (TraceFlags & TRACE_Level) ? LevelPtr : NULL,
		End, Start, Extent, (TraceFlags & ~TRACE_Level) != 0, Extra );
	while( Check )
	{
		AActor* Other = Check->Actor;
		if( !Source )
		{
			if( Other->IsA(ALevelInfo::StaticClass()) )
			{
				if( TraceFlags & TRACE_Level ) break;
			}
			else if( Other->IsA(AMover::StaticClass()) )
			{
				if( TraceFlags & TRACE_Movers ) break;
			}
			else if( Other->IsA(AZoneInfo::StaticClass()) )
			{
				if( TraceFlags & TRACE_ZoneChanges ) break;
			}
			else if( TraceFlags & TRACE_Others )
				break;
		}
		Check = Check->GetNext();
	}
	if( Check )
	{
		*Hit = *Check;
		return 0;
	}
	Hit->Time = 1.f;
	Hit->Actor = NULL;
	return 1;
}

UClass* AActor::PrivateStaticClass = (UClass*)g_cls_raw[0];
UClass* ALevelInfo::PrivateStaticClass = (UClass*)g_cls_raw[1];
UClass* AMover::PrivateStaticClass = (UClass*)g_cls_raw[2];
UClass* AZoneInfo::PrivateStaticClass = (UClass*)g_cls_raw[3];
UClass* ABrush::PrivateStaticClass = (UClass*)g_cls_raw[4];
UClass* AProjectile::PrivateStaticClass = (UClass*)g_cls_raw[5];
UClass* APlayerPawn::PrivateStaticClass = (UClass*)g_cls_raw[6];
UClass* APawn::PrivateStaticClass = (UClass*)g_cls_raw[7];

UClass* APawn::GetPrivateStaticClassAPawn( const TCHAR* Package )
{
	(void)Package;
	return PrivateStaticClass;
}

void APawn::InitializePrivateStaticClassAPawn()
{
}
FMemStack GMem;

DWORD appCycles() { return 0; }

// FMemStack chunk allocation is never reached: the per-case buffer is large enough.
BYTE* FMemStack::AllocateNewChunk( INT MinSize ) { abort(); }
static int actor_index(const AActor* A);
static char g_hook_log[65536];
static int g_hook_count;
static int g_hook_used;

static void clear_hooks()
{
	g_hook_count = 0;
	g_hook_used = 0;
	g_hook_log[0] = 0;
}

static void log_hook(const char* Format, ...)
{
	va_list Args;
	va_start(Args, Format);
	if( g_hook_used && g_hook_used < (int)sizeof(g_hook_log) - 1 )
		g_hook_log[g_hook_used++] = ' ';
	g_hook_used += vsnprintf(g_hook_log + g_hook_used, sizeof(g_hook_log) - g_hook_used, Format, Args);
	va_end(Args);
	g_hook_count++;
}

static void log_hit(const char* Prefix, const FCheckResult& Hit)
{
	log_hook("%s %08x %08x %08x %08x %08x %08x %08x %d %d",
		Prefix, bits(Hit.Time), bits(Hit.Location.X), bits(Hit.Location.Y), bits(Hit.Location.Z),
		bits(Hit.Normal.X), bits(Hit.Normal.Y), bits(Hit.Normal.Z),
		actor_index(Hit.Actor), Hit.Item);
}
static int hit_count( FCheckResult* Hit )
{
	int Count = 0;
	for( FCheckResult* P = Hit; P; P = P->GetNext() )
		Count++;
	return Count;
}

static void append_hit_text( char* Text, size_t Capacity, int& Used, const FCheckResult& Hit )
{
	if( Used < (int)Capacity )
		Used += snprintf( Text + Used, Capacity - Used, " %08x %08x %08x %08x %08x %08x %08x %d %d",
			bits(Hit.Time), bits(Hit.Location.X), bits(Hit.Location.Y), bits(Hit.Location.Z),
			bits(Hit.Normal.X), bits(Hit.Normal.Y), bits(Hit.Normal.Z), actor_index(Hit.Actor), Hit.Item );
}

static void log_actor_point( FVector Location, FVector Extent, DWORD Flags, FCheckResult* Head )
{
	char Text[4096];
	int Used = snprintf( Text, sizeof Text, "AP %08x %08x %08x %08x %08x %08x %08x %d",
		bits(Location.X), bits(Location.Y), bits(Location.Z), bits(Extent.X), bits(Extent.Y),
		bits(Extent.Z), (uint32_t)Flags, hit_count(Head) );
	for( FCheckResult* P = Head; P; P = P->GetNext() )
		append_hit_text( Text, sizeof Text, Used, *P );
	log_hook( "%s", Text );
}

static void log_actor_line( FVector End, FVector Start, FVector Extent, BYTE Flags, FCheckResult* Head )
{
	char Text[4096];
	int Used = snprintf( Text, sizeof Text, "AL %08x %08x %08x %08x %08x %08x %08x %08x %08x %02x %d",
		bits(End.X), bits(End.Y), bits(End.Z), bits(Start.X), bits(Start.Y), bits(Start.Z),
		bits(Extent.X), bits(Extent.Y), bits(Extent.Z), (uint32_t)Flags, hit_count(Head) );
	for( FCheckResult* P = Head; P; P = P->GetNext() )
		append_hit_text( Text, sizeof Text, Used, *P );
	log_hook( "%s", Text );
}

static void log_actor_encroachment( AActor* Actor, FVector Location, FRotator Rotation, DWORD Flags, FCheckResult* Head )
{
	char Text[4096];
	int Used = snprintf( Text, sizeof Text, "EC %d %08x %08x %08x %d %d %d %08x %d",
		actor_index(Actor), bits(Location.X), bits(Location.Y), bits(Location.Z),
		Rotation.Yaw, Rotation.Roll, Rotation.Pitch, (uint32_t)Flags, hit_count(Head) );
	for( FCheckResult* P = Head; P; P = P->GetNext() )
		append_hit_text( Text, sizeof Text, Used, *P );
	log_hook( "%s", Text );
}


static void print_hooks()
{
	printf(" | %d%s", g_hook_count, g_hook_used ? " " : "");
	if( g_hook_used )
		printf("%s", g_hook_log);
}

 

static FCheckResult* hook_aec( FCollisionHashBase* Self, FMemStack& Mem, AActor* Actor, FVector Location, FRotator Rotation, DWORD Flags )
{
	FCheckResult* Hit = NULL;
	if( !g_empty_encroachment )
	{
		Hit = new(Mem) FCheckResult(1.f);
		Hit->Location = FVector(2.f, 3.f, 4.f);
		Hit->Normal = FVector(0.f, 1.f, 0.f);
		Hit->Actor = g_pool[1];
		Hit->Item = 17;
		Hit->GetNext() = NULL;
	}
	if( g_actor_ops )
		log_actor_encroachment( Actor, Location, Rotation, Flags, Hit );
	return Hit;
}

static FCheckResult* hook_ahpc( FCollisionHashBase* Self, FMemStack& Mem, FVector Location, FVector Extent, DWORD Flags )
{
	uint64_t h = hmix(hvec(hvec(hmix(0x31, 0), Location), Extent), Flags);
	int n = (int)(h % 4);
	FCheckResult* Head = NULL;
	for( int i = n - 1; i >= 0; i-- )
	{
		uint64_t hi = hmix(h, 0x100 + (uint64_t)i);
		FCheckResult* Node = new(Mem) FCheckResult(1.f);
		fill_hit( *Node, hi );
		Node->Actor = g_pool[draw(hi, 9) % 5];
		Node->GetNext() = Head;
		Head = Node;
	}
	if( g_actor_ops )
		log_actor_point( Location, Extent, Flags, Head );
	return Head;
}


static FCheckResult* hook_ahlc( FCollisionHashBase* Self, FMemStack& Mem, FVector End, FVector Start, FVector Extent, BYTE Flags )
{
	if( g_force_self_hit )
	{
		FCheckResult* Node = new(Mem) FCheckResult( 0.25f );
		Node->Actor = g_pool[0];
		Node->Location = FVector( 1.f, 2.f, 3.f );
		Node->Normal = FVector( 0.f, 0.f, 1.f );
		Node->Item = 7;
		Node->GetNext() = NULL;
		log_actor_line( End, Start, Extent, Flags, Node );
		return Node;
	}
	if( g_move_center_mode )
	{
		log_actor_line( End, Start, Extent, Flags, NULL );
		return NULL;
	}
	uint64_t h = hmix(hvec(hvec(hvec(hmix(0x41, 0), End), Start), Extent), (uint64_t)Flags);
	int n = (int)(h % 4);
	FCheckResult* Head = NULL;
	for( int i = n - 1; i >= 0; i-- )
	{
		uint64_t hi = hmix(h, 0x100 + (uint64_t)i);
		FCheckResult* Node = new(Mem) FCheckResult(1.f);
		fill_hit( *Node, hi );
		Node->Actor = g_pool[draw(hi, 9) % 5];
		Node->GetNext() = Head;
		Head = Node;
	}
	if( g_actor_ops )
		log_actor_line( End, Start, Extent, Flags, Head );
	return Head;
}
static void hook_hash_add( FCollisionHashBase* Self, AActor* Actor )
{
	if( g_actor_ops )
		log_hook( "HA %d", actor_index(Actor) );
}

static void hook_hash_remove( FCollisionHashBase* Self, AActor* Actor, bool bThorough )
{
	if( g_actor_ops )
		log_hook( "HR %d %d", actor_index(Actor), (int)bThorough );
}

static UBOOL hook_pc( UModel* Self, FCheckResult& Result, AActor* Owner, FVector Location, FVector Extent, DWORD Flags )
{
	uint64_t h = hvec(hvec(hmix(0x11, 0), Location), Extent);
	UBOOL R = h % 3 == 0;
	if( !R )
		fill_hit( Result, hmix(h, 0x12) );
	if( g_actor_ops )
		log_hook("PC %08x %08x %08x %08x %08x %08x %08x %d %d %08x %08x %08x %08x %08x %08x %08x %d %d",
			bits(Location.X), bits(Location.Y), bits(Location.Z),
			bits(Extent.X), bits(Extent.Y), bits(Extent.Z), (uint32_t)Flags,
			actor_index(Owner), (int)R,
			bits(Result.Time), bits(Result.Location.X), bits(Result.Location.Y), bits(Result.Location.Z),
			bits(Result.Normal.X), bits(Result.Normal.Y), bits(Result.Normal.Z),
			actor_index(Result.Actor), Result.Item);
	return R;
}

static UBOOL hook_lc( UModel* Self, FCheckResult& Result, AActor* Owner, FVector End, FVector Start, FVector Extent, DWORD Flags )
{
	if( g_move_center_mode )
	{
		g_trace_start = Start;
		g_trace_end = End;
		g_trace_calls++;
		Result.Time = 1.f;
		Result.Location = FVector(0.f, 0.f, 0.f);
		Result.Normal = FVector(0.f, 0.f, 0.f);
		Result.Item = 0;
		Result.Actor = NULL;
		Result.GetNext() = NULL;
		log_hook("LC %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %d %d %08x %08x %08x %08x %08x %08x %08x %d %d",
			bits(End.X), bits(End.Y), bits(End.Z), bits(Start.X), bits(Start.Y), bits(Start.Z),
			bits(Extent.X), bits(Extent.Y), bits(Extent.Z), (uint32_t)Flags,
			actor_index(Owner), 1, bits(Result.Time), bits(Result.Location.X), bits(Result.Location.Y), bits(Result.Location.Z),
			bits(Result.Normal.X), bits(Result.Normal.Y), bits(Result.Normal.Z), actor_index(Result.Actor), Result.Item);
		return 1;
	}
	uint64_t h = hmix(hvec(hvec(hvec(hmix(0x21, 0), End), Start), Extent), (uint64_t)Flags);
	UBOOL R = h % 3 == 0;
	if( !R )
		fill_hit( Result, hmix(h, 0x22) );
	if( g_actor_ops )
		log_hook("LC %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %d %d %08x %08x %08x %08x %08x %08x %08x %d %d",
			bits(End.X), bits(End.Y), bits(End.Z), bits(Start.X), bits(Start.Y), bits(Start.Z),
			bits(Extent.X), bits(Extent.Y), bits(Extent.Z), (uint32_t)Flags,
			actor_index(Owner), (int)R, bits(Result.Time), bits(Result.Location.X), bits(Result.Location.Y), bits(Result.Location.Z),
			bits(Result.Normal.X), bits(Result.Normal.Y), bits(Result.Normal.Z), actor_index(Result.Actor), Result.Item);
	return R;
}

// ULevel virtual slots used by the original code: forwarders to the real member bodies.
static FCheckResult* fwd_mpc( ULevel* Self, FMemStack& Mem, FVector Location, FVector Extent, DWORD Flags, ALevelInfo* Lv, UBOOL bActors )
{
	return Self->ULevel::MultiPointCheck( Mem, Location, Extent, Flags, Lv, bActors );
}
static FCheckResult* fwd_mlc( ULevel* Self, FMemStack& Mem, FVector End, FVector Start, FVector Extent, UBOOL bCheckActors, ALevelInfo* Lv, BYTE Flags )
{
	return Self->ULevel::MultiLineCheck( Mem, End, Start, Extent, bCheckActors, Lv, Flags );
}
static void fwd_saz( ULevel* Self, AActor* Actor, UBOOL bTest, UBOOL bForceRefresh )
{
	Self->ULevel::SetActorZone( Actor, bTest, bForceRefresh );
}
static UBOOL fwd_spc( ULevel* Self, FCheckResult& Hit, FVector Location, FVector Extent, DWORD ExtraNodeFlags, ALevelInfo* Level, UBOOL bActors )
{
	return Self->ULevel::SinglePointCheck( Hit, Location, Extent, ExtraNodeFlags, Level, bActors );
}
static UBOOL fwd_slc( ULevel* Self, FCheckResult& Hit, AActor* SourceActor, const FVector& End, const FVector& Start, DWORD TraceFlags, FVector Extent, BYTE ExtraNodeFlags )
{
	return Self->ULevel::SingleLineCheck( Hit, SourceActor, End, Start, TraceFlags, Extent, ExtraNodeFlags );
}
static void fwd_adj( ULevel* Self, FVector& Adjusted, FVector TraceDest, FLOAT TraceLen, FCheckResult& Hit )
{
	Self->ULevel::AdjustSpot( Adjusted, TraceDest, TraceLen, Hit );
}
static UBOOL fwd_ec( ULevel* Self, AActor* Actor, FVector TestLocation, FRotator TestRotation, UBOOL bTouchNotify )
{
	return Self->ULevel::CheckEncroachment( Actor, TestLocation, TestRotation, bTouchNotify );
}
static UBOOL fwd_move( ULevel* Self, AActor* Actor, FVector Delta, FRotator NewRotation, FCheckResult& Hit, UBOOL Test, UBOOL IgnorePawns, UBOOL IgnoreBases, UBOOL NoFail )
{
	return Self->ULevel::MoveActor( Actor, Delta, NewRotation, Hit, Test, IgnorePawns, IgnoreBases, NoFail );
}

static UBOOL fwd_find_spot( ULevel* Self, FVector Extent, FVector& Location, UBOOL bCheckActors, UBOOL bAssumeFit )
{
	return Self->ULevel::FindSpot( Extent, Location, bCheckActors, bAssumeFit );
}

static void* g_lvl_vt[512];
static void* g_hash_vt[512];
static void* g_model_vt[512];

// Vtable slot of a virtual member function pointer (ARM member-pointer ABI: word0 is the byte
// offset of the vtable entry, word1 bit0 marks a virtual member).
static size_t vt_slot( const void* Pm )
{
	uintptr_t W[2];
	memcpy( W, Pm, sizeof W );
	if( !(W[1] & 1) )
		abort();
	return W[0] / sizeof(void*);
}

static void set_vptr( void* Obj, void** Vt )
{
	memcpy( Obj, &Vt, sizeof Vt );
}
static UPrimitive* hook_get_primitive( const AActor* Self )
{
	log_hook("GP %d", actor_index(Self));
	return g_test_primitive;
}

static FBox hook_collision_box( UPrimitive* Self, const AActor* Owner, bool bWorld )
{
	FBox Box( FVector( 10.f, 20.f, 30.f ), FVector( 14.f, 26.f, 42.f ) );
	log_hook("CB %d %d %08x %08x %08x %08x %08x %08x",
		actor_index(Owner), (int)bWorld,
		bits(Box.Min.X), bits(Box.Min.Y), bits(Box.Min.Z),
		bits(Box.Max.X), bits(Box.Max.Y), bits(Box.Max.Z));
	return Box;
}

static void init_world()
{
	for( int K = 1; K < 8; K++ )
		((UClass*)g_cls_raw[K])->SuperField = (UClass*)g_cls_raw[K == 6 ? 7 : 0];

	for( int K = 0; K < 6; K++ )
	{
		g_pool[K] = (AActor*)g_obj_raw[K];
		g_pool[K]->Class = (UClass*)g_cls_raw[K == 3 ? 2 : K == 4 ? 3 : K == 5 ? 1 : 0];
	}
	g_levinfo = (ALevelInfo*)g_pool[5];

	auto PmMpc = &ULevel::MultiPointCheck;
	auto PmMlc = &ULevel::MultiLineCheck;
	auto PmAhpc = &FCollisionHashBase::ActorPointCheck;
	auto PmSaz = &ULevel::SetActorZone;
	auto PmAhlc = &FCollisionHashBase::ActorLineCheck;
	auto PmAec = &FCollisionHashBase::ActorEncroachmentCheck;
	auto PmSpc = &ULevel::SinglePointCheck;
	auto PmSlc = &ULevel::SingleLineCheck;
	auto PmAddActor = &FCollisionHashBase::AddActor;
	auto PmRemoveActor = &FCollisionHashBase::RemoveActor;
	auto PmPc = &UModel::PointCheck;
	auto PmLc = &UModel::LineCheck;
	auto PmAdj = &ULevel::AdjustSpot;
	auto PmFindSpot = &ULevel::FindSpot;
	auto PmEc = &ULevel::CheckEncroachment;
	g_lvl_vt[vt_slot( &PmMpc )] = (void*)&fwd_mpc;
	auto PmMove = &ULevel::MoveActor;
	g_lvl_vt[vt_slot( &PmMlc )] = (void*)&fwd_mlc;
	g_lvl_vt[vt_slot( &PmSaz )] = (void*)&fwd_saz;
	g_lvl_vt[vt_slot( &PmFindSpot )] = (void*)&fwd_find_spot;
	g_lvl_vt[vt_slot( &PmSpc )] = (void*)&fwd_spc;
	g_hash_vt[vt_slot( &PmAhpc )] = (void*)&hook_ahpc;
	g_lvl_vt[vt_slot( &PmSlc )] = (void*)&fwd_slc;
	g_lvl_vt[vt_slot( &PmAdj )] = (void*)&fwd_adj;
	g_lvl_vt[vt_slot( &PmEc )] = (void*)&fwd_ec;
	g_hash_vt[vt_slot( &PmAec )] = (void*)&hook_aec;
	g_hash_vt[vt_slot( &PmAddActor )] = (void*)&hook_hash_add;
	g_lvl_vt[vt_slot( &PmMove )] = (void*)&fwd_move;
	g_hash_vt[vt_slot( &PmRemoveActor )] = (void*)&hook_hash_remove;
	g_hash_vt[vt_slot( &PmAhlc )] = (void*)&hook_ahlc;
	g_model_vt[vt_slot( &PmPc )] = (void*)&hook_pc;
	g_model_vt[vt_slot( &PmLc )] = (void*)&hook_lc;
	auto PmGetPrimitive = &AActor::GetPrimitive;
	auto PmCollisionBox = &UPrimitive::GetCollisionBoundingBox;
	g_actor_vt[vt_slot( &PmGetPrimitive )] = (void*)&hook_get_primitive;
	g_primitive_vt[vt_slot( &PmCollisionBox )] = (void*)&hook_collision_box;

	g_lvl = (ULevel*)calloc( 1, sizeof(ULevel) );
	g_model = (UModel*)calloc( 1, sizeof(UModel) );
	g_hash = (FCollisionHashBase*)calloc( 1, sizeof(FCollisionHashBase) + 64 );
	set_vptr( g_lvl, g_lvl_vt );
	set_vptr( g_model, g_model_vt );
	set_vptr( g_hash, g_hash_vt );
	g_test_primitive = (UPrimitive*)g_primitive_raw;
	set_vptr( g_test_primitive, g_primitive_vt );
	g_lvl->Model = g_model;
	g_lvl->Hash = g_hash;
	g_actors[0] = g_pool[5];
	g_lvl->Actors.Data = g_actors;
	g_lvl->Actors.ArrayNum = 1;
	for( int K = 0; K < 6; K++ )
		g_pool[K]->XLevel = g_lvl;
}

static int actor_index( const AActor* A )
{
	if( !A ) return -1;
	for( int K = 0; K < 6; K++ )
		if( g_pool[K] == A ) return K;
	return -2;
}
FPointRegion UModel::PointRegion( AZoneInfo* Zone, FVector Location ) const
{
	FPointRegion Result( Zone, INDEX_NONE, 0 );
	log_hook( "ZP %d %08x %08x %08x %d %d %02x",
		actor_index(Zone), bits(Location.X), bits(Location.Y), bits(Location.Z),
		actor_index(Result.Zone), Result.iLeaf, (uint32_t)Result.ZoneNumber );
	return Result;
}


static void put_vec( FVector V ) { printf( " %08x %08x %08x", bits(V.X), bits(V.Y), bits(V.Z) ); }

static void put_hit( const FCheckResult& H )
{
	printf( " %08x", bits(H.Time) );
	put_vec( H.Location );
	put_vec( H.Normal );
	printf( " %d %d", actor_index( H.Actor ), H.Item );
}

static void put_list( FCheckResult* H )
{
	int N = 0;
	for( FCheckResult* P = H; P; P = P->GetNext() )
		N++;
	printf( " %d", N );
	for( FCheckResult* P = H; P; P = P->GetNext() )
		put_hit( *P );
}

static void check_cases()
{
	const int PerKind = 1500;
	for( int I = 0; I < 4 * PerKind; I++ )
	{
		int Kind = I % 4;
		FVector Loc( rand_coord(), rand_coord(), rand_coord() );
		FVector End( rand_coord(), rand_coord(), rand_coord() );
		FVector Start( rand_coord(), rand_coord(), rand_coord() );
		FVector Ext( rand_coord(), rand_coord(), rand_coord() );
		DWORD Flags = next_u32() & 0xFFFF;
		BYTE Node = (BYTE)(next_u32() & 0xFF);
		int bActors = next_u32() & 1;
		int Lvl = next_u32() & 1;
		int Src = (int)(next_u32() % 6) - 1;
		int Own[5], Blk[5];
		for( int K = 0; K < 5; K++ )
		{
			Own[K] = (int)(next_u32() % (K + 1)) - 1;   // acyclic: owner index < K
			Blk[K] = next_u32() & 1;
		}
		for( int K = 0; K < 5; K++ )
		{
			g_pool[K]->Owner = Own[K] >= 0 ? g_pool[Own[K]] : NULL;
			g_pool[K]->bBlockActors = Blk[K];
		}
		GMem.Top = (BYTE*)g_membuf;
		GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
		ALevelInfo* Lv = Lvl ? g_levinfo : NULL;

		printf( "chk %d", Kind );
		put_vec( Loc ); put_vec( End ); put_vec( Start ); put_vec( Ext );
		printf( " %08x %02x %d %d %d", (uint32_t)Flags, (uint32_t)Node, bActors, Lvl, Src );
		for( int K = 0; K < 5; K++ ) printf( " %d", Own[K] );
		for( int K = 0; K < 5; K++ ) printf( " %d", Blk[K] );
		printf( " ->" );

		if( Kind == 0 )
		{
			FCheckResult Hit( 3.f );
			Hit.Location = FVector( 7.f, 8.f, 9.f );
			Hit.Normal = FVector( -7.f, -8.f, -9.f );
			Hit.Item = -3;
			Hit.Actor = NULL;
			UBOOL R = g_lvl->ULevel::SinglePointCheck( Hit, Loc, Ext, Flags, Lv, bActors );
			printf( " %d", R );
			put_hit( Hit );
		}
		else if( Kind == 1 )
		{
			FCheckResult* H = g_lvl->ULevel::MultiPointCheck( GMem, Loc, Ext, Flags, Lv, bActors );
			put_list( H );
		}
		else if( Kind == 2 )
		{
			FCheckResult Hit( 3.f );
			Hit.Location = FVector( 7.f, 8.f, 9.f );
			Hit.Normal = FVector( -7.f, -8.f, -9.f );
			Hit.Item = -3;
			Hit.Actor = NULL;
			AActor* Source = Src >= 0 ? g_pool[Src] : NULL;
			UBOOL R = g_lvl->ULevel::SingleLineCheck( Hit, Source, End, Start, Flags, Ext, Node );
			printf( " %d", R );
			put_hit( Hit );
		}
		else
		{
			FCheckResult* H = g_lvl->ULevel::MultiLineCheck( GMem, End, Start, Ext, bActors, Lv, Node );
			put_list( H );
		}
		printf( "\n" );
	}
}

static int actor_class(const AActor* A)
{
	if( A->Class == (UClass*)g_cls_raw[2] ) return 1;
	if( A->Class == (UClass*)g_cls_raw[3] ) return 2;
	if( A->Class == (UClass*)g_cls_raw[1] ) return 3;
	if( A->Class == (UClass*)g_cls_raw[4] ) return 5;
	if( A->Class == (UClass*)g_cls_raw[5] ) return 6;
	return A->bIsPawn ? 4 : 0;
}

static uint32_t actor_flags(const AActor* A)
{
	return (A->bStatic ? 1u : 0u) | (A->bMovable ? 2u : 0u)
		| (A->bCollideActors ? 4u : 0u) | (A->bCollideWorld ? 8u : 0u)
		| (A->bBlockActors ? 16u : 0u) | (A->bBlockPlayers ? 32u : 0u)
		| (A->bIsPawn ? 64u : 0u) | (A->IsBrush() ? 128u : 0u)
		| (A->bDeleteMe ? 256u : 0u) | (A->bJustTeleported ? 512u : 0u);
}

static void put_region(const FPointRegion& R)
{
	printf(" %d %d %02x", actor_index(R.Zone), R.iLeaf, (uint32_t)R.ZoneNumber);
}

static void put_actor_row(const AActor* A)
{
	printf(" %d %d %d", actor_class(A), actor_index(A->Owner), actor_index(A->Base));
	put_region(A->Region);
	printf(" %08x %08x", actor_flags(A), (uint32_t)A->Tag.GetIndex());
	put_vec(A->Location);
	put_vec(A->OldLocation);
	printf(" %d", (int)A->bJustTeleported);
	printf(" %d %d %d", A->Rotation.Yaw, A->Rotation.Roll, A->Rotation.Pitch);
	put_vec(A->Velocity);
	printf(" %08x %08x %08x %d", bits(A->CollisionRadius), bits(A->CollisionWidth),
		bits(A->CollisionHeight), (int)A->StandingCount);
	if( A->bIsPawn )
	{
		const APawn* P = (const APawn*)A;
		put_region(P->FootRegion);
		put_region(P->HeadRegion);
		printf(" %08x", bits(P->EyeHeight));
	}
	else
		printf(" -1 -1 00 -1 -1 00 00000000");
}

static void put_actor_rows()
{
	for( int K = 0; K < 6; K++ )
		put_actor_row( g_pool[K] );
}

static void begin_actor_record( const char* Op, int Id, const char* Branch, int LevelIndex )
{
	printf("%s %d %s 6 %d %d", Op, Id, Branch, LevelIndex, g_lvl->Actors.ArrayNum);
	for( int K = 0; K < g_lvl->Actors.ArrayNum; K++ )
		printf(" %d", actor_index(g_actors[K]));
	put_actor_rows();
}


static void far_move_case( int Id, const char* Branch, UBOOL bNoCheck )
{
	AActor* A = g_pool[0];
	AActor* Other = g_pool[1];
	g_actor_ops = true;
	g_actors[0] = g_levinfo;
	g_actors[1] = NULL;
	g_lvl->Actors.ArrayNum = 1;
	A->Class = (UClass*)g_cls_raw[0];
	A->bIsPawn = 0;
	A->bDeleteMe = 0;
	A->bStatic = 0;
	A->bMovable = 1;
	A->bCollideActors = 1;
	A->bCollideWorld = !bNoCheck;
	A->bCollideWhenPlacing = 0;
	A->bBlockActors = 0;
	A->bBlockPlayers = 0;
	A->StandingCount = 0;
	A->Base = NULL;
	A->Location = FVector(1.f, 2.f, 3.f);
	A->OldLocation = FVector(0.f, 0.f, 0.f);
	A->bJustTeleported = 0;
	A->CollisionRadius = A->CollisionWidth = 1.f;
	A->CollisionHeight = 1.f;
	A->Rotation = FRotator(11, 22, 33);
	A->Region = FPointRegion(g_levinfo);
	Other->Class = (UClass*)g_cls_raw[0];
	Other->bIsPawn = 0;
	Other->bBlockActors = 0;
	Other->bBlockPlayers = 0;
	Other->bCollideWorld = 0;
	GMem.Top = (BYTE*)g_membuf;
	GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
	clear_hooks();
	g_empty_encroachment = true;
	FVector DestLocation(9.f, 8.f, 7.f);
	printf("far_move %d %s %d 6 %d %d", Id, Branch, actor_index(A), actor_index(g_levinfo), g_lvl->Actors.ArrayNum);
	for( int K = 0; K < g_lvl->Actors.ArrayNum; K++ )
		printf(" %d", actor_index(g_actors[K]));
	put_actor_rows();
	put_vec(DestLocation);
	printf(" 0 %d ->", (int)bNoCheck);
	UBOOL Moved = g_lvl->ULevel::FarMoveActor(A, DestLocation, 0, bNoCheck);
	g_empty_encroachment = false;
	printf(" %d 6", (int)Moved);
	put_actor_rows();
	print_hooks();
	printf("\n");
}
static void actor_operation_cases()
{
	g_actor_ops = true;
	AActor* A = g_pool[0];
	FCheckResult Hit(7.f);
	Hit.Location = FVector(8.f, 9.f, 10.f);
	Hit.Normal = FVector(11.f, 12.f, 13.f);
	Hit.Item = 14;
	Hit.Actor = NULL;
	A->Class = (UClass*)g_cls_raw[0];
	A->bIsPawn = 0;
	A->bDeleteMe = 0;
	A->bStatic = 1;
	A->bMovable = 1;
	A->bCollideActors = A->bCollideWorld = A->bBlockActors = A->bBlockPlayers = 0;
	A->Location = FVector(1.f, 2.f, 3.f);
	A->Rotation = FRotator(11, 22, 33);
	A->Velocity = FVector(4.f, 5.f, 6.f);
	begin_actor_record("move_actor", 0, "static", actor_index(g_levinfo));
	printf(" 0");
	put_vec(FVector(1.f, 0.f, 0.f));
	printf(" 55 66 44");
	put_hit(Hit);
	printf(" 0 0 0 0 ->");
	UBOOL Moved = g_lvl->ULevel::MoveActor(A, FVector(1.f, 0.f, 0.f),
		FRotator(44, 55, 66), Hit, 0, 0, 0, 0);
	printf(" %d", (int)Moved);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	printf(" | 0\n");

	A->bStatic = 0;
	A->Location = FVector(1.f, 2.f, 3.f);
	A->Rotation = FRotator(11, 22, 33);
	Hit = FCheckResult(7.f);
	Hit.Location = FVector(8.f, 9.f, 10.f);
	Hit.Normal = FVector(11.f, 12.f, 13.f);
	Hit.Item = 14;
	Hit.Actor = NULL;
	begin_actor_record("move_actor", 1, "zero_same", actor_index(g_levinfo));
	printf(" 0");
	put_vec(FVector(0.f, 0.f, 0.f));
	printf(" 22 33 11");
	put_hit(Hit);
	printf(" 0 0 0 0 ->");
	Moved = g_lvl->ULevel::MoveActor(A, FVector(0.f, 0.f, 0.f), A->Rotation,
		Hit, 0, 0, 0, 0);
	printf(" %d", (int)Moved);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	printf(" | 0\n");

	A->Rotation = FRotator(11, 22, 33);
	Hit = FCheckResult(7.f);
	Hit.Location = FVector(8.f, 9.f, 10.f);
	Hit.Normal = FVector(11.f, 12.f, 13.f);
	Hit.Item = 14;
	Hit.Actor = NULL;
	begin_actor_record("move_actor", 2, "zero_rotate", actor_index(g_levinfo));
	printf(" 0");
	put_vec(FVector(0.f, 0.f, 0.f));
	printf(" 5 6 4");
	put_hit(Hit);
	printf(" 0 0 0 0 ->");
	Moved = g_lvl->ULevel::MoveActor(A, FVector(0.f, 0.f, 0.f), FRotator(4, 5, 6),
		Hit, 0, 0, 0, 0);
	printf(" %d", (int)Moved);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	printf(" | 0\n");

	A->Location = FVector(1.f, 2.f, 3.f);
	A->Rotation = FRotator(11, 22, 33);
	A->Region = FPointRegion(g_levinfo);
	Hit = FCheckResult(7.f);
	Hit.Location = FVector(8.f, 9.f, 10.f);
	Hit.Normal = FVector(11.f, 12.f, 13.f);
	Hit.Item = 14;
	Hit.Actor = NULL;
	FCheckResult OriginalHit = Hit;
	GMem.Top = (BYTE*)g_membuf;
	GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
	clear_hooks();
	FVector Delta(1.f, 0.f, 0.f);
	begin_actor_record("move_actor", 3, "no_collision", actor_index(g_levinfo));
	printf(" 0");
	put_vec(Delta);
	printf(" 22 33 11");
	put_hit(OriginalHit);
	printf(" 1 0 0 0 ->");
	Moved = g_lvl->ULevel::MoveActor(A, Delta, A->Rotation, Hit, 1, 0, 0, 0);
	printf(" %d", (int)Moved);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	print_hooks();
	printf("\n");

	FVector TestLocation(2.f, 3.f, 4.f);
	FRotator TestRotation(1, 2, 3);
	GMem.Top = (BYTE*)g_membuf;
	GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
	A->bDeleteMe = 0;
	A->Class = (UClass*)g_cls_raw[0];
	A->bIsPawn = 0;
	A->bCollideActors = 1;
	A->bBlockActors = 0;
	A->bBlockPlayers = 0;
	A->Region = FPointRegion(g_levinfo);
	clear_hooks();
	begin_actor_record("check_encroachment", 1, "nonblocking", actor_index(g_levinfo));
	printf(" 0");
	put_vec(TestLocation);
	printf(" 2 3 1 0 ->");
	UBOOL Encroached = g_lvl->ULevel::CheckEncroachment(A, TestLocation, TestRotation, 0);
	printf(" %d 6", (int)Encroached);
	put_actor_rows();
	print_hooks();
	printf("\n");

	A->bCollideActors = A->bBlockActors = A->bBlockPlayers = A->bCollideWorld = 0;
	clear_hooks();
	begin_actor_record("check_encroachment", 0, "skip", actor_index(g_levinfo));
	printf(" 0");
	put_vec(TestLocation);
	printf(" 2 3 1 1 -> %d 6", (int)g_lvl->ULevel::CheckEncroachment(A, TestLocation, TestRotation, 1));
	put_actor_rows();
	printf(" | 0\n");

	A->bDeleteMe = 1;
	clear_hooks();
	begin_actor_record("set_actor_zone", 0, "delete", actor_index(g_levinfo));
	printf(" 0 0 0 -> - 6");
	g_lvl->ULevel::SetActorZone(A, 0, 0);
	put_actor_rows();
	printf(" | 0\n");

	A->bDeleteMe = 0;
	A->Region = FPointRegion(g_levinfo);
	clear_hooks();
	begin_actor_record("set_actor_zone", 1, "zone_same", actor_index(g_levinfo));
	printf(" 0 0 0 -> - 6");
	g_lvl->ULevel::SetActorZone(A, 0, 0);
	put_actor_rows();
	print_hooks();
	printf("\n");

	A->Region = FPointRegion(g_levinfo);
	clear_hooks();
	begin_actor_record("set_actor_zone", 4, "force_refresh_same", actor_index(g_levinfo));
	printf(" 0 0 1 -> - 6");
	g_lvl->ULevel::SetActorZone(A, 0, 1);
	put_actor_rows();
	print_hooks();
	printf("\n");
	A->Class = (UClass*)g_cls_raw[6];
	A->bIsPawn = 1;
	A->Region = FPointRegion(g_levinfo);
	((APawn*)A)->FootRegion = FPointRegion(g_levinfo);
	((APawn*)A)->HeadRegion = FPointRegion(g_levinfo);
	((APawn*)A)->EyeHeight = 16.f;
	A->CollisionHeight = 8.f;
	clear_hooks();
	begin_actor_record("set_actor_zone", 2, "pawn_regions", actor_index(g_levinfo));
	printf(" 0 0 1 -> - 6");
	g_lvl->ULevel::SetActorZone(A, 0, 1);
	put_actor_rows();
	print_hooks();
	printf("\n");

	g_actors[0] = A;
	A->Class = (UClass*)g_cls_raw[1];
	A->bIsPawn = 0;
	A->Region = FPointRegion();
	clear_hooks();
	begin_actor_record("set_actor_zone", 3, "level_actor", actor_index(A));
	printf(" 0 0 0 -> - 6");
	g_lvl->ULevel::SetActorZone(A, 0, 0);
	put_actor_rows();
	printf(" | 0\n");
	g_actors[0] = g_levinfo;

	{
		GMem.Top = (BYTE*)g_membuf;
		GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
		FVector Extent(0.f, 0.f, 0.f);
		FVector Location(1.f, 2.f, 3.f);
		FVector Probe = Location;
		clear_hooks();
		UBOOL Fit = g_lvl->ULevel::FindSpot(Extent, Probe, 0, 0);
		begin_actor_record("find_spot", 0, Fit ? "zero_fit" : "zero_block", actor_index(g_levinfo));
		put_vec(Extent);
		put_vec(Location);
		printf(" 0 0 -> %d", (int)Fit);
		put_vec(Probe);
		printf(" 6");
		put_actor_rows();
		print_hooks();
		printf("\n");

		Location = FVector(1.f, 0.f, 0.f);
		Probe = Location;
		GMem.Top = (BYTE*)g_membuf;
		GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
		clear_hooks();
		Fit = g_lvl->ULevel::FindSpot(Extent, Probe, 0, 0);
		begin_actor_record("find_spot", 1, Fit ? "zero_fit" : "zero_block", actor_index(g_levinfo));
		put_vec(Extent);
		put_vec(Location);
		printf(" 0 0 -> %d", (int)Fit);
		put_vec(Probe);
		printf(" 6");
		put_actor_rows();
		print_hooks();
		printf("\n");

		Extent = FVector(1.f, 1.f, 1.f);
		Location = FVector(1.f, 0.f, 0.f);
		Probe = Location;
		GMem.Top = (BYTE*)g_membuf;
		GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
		clear_hooks();
		Fit = g_lvl->ULevel::FindSpot(Extent, Probe, 0, 1);
		begin_actor_record("find_spot", 2, "assume_fit", actor_index(g_levinfo));
		put_vec(Extent);
		put_vec(Location);
		printf(" 0 1 -> %d", (int)Fit);
		put_vec(Probe);
		printf(" 6");
		put_actor_rows();
		print_hooks();
		printf("\n");

		Location = FVector(2.f, 3.f, 4.f);
		Probe = Location;
		GMem.Top = (BYTE*)g_membuf;
		GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
		clear_hooks();
		Fit = g_lvl->ULevel::FindSpot(Extent, Probe, 0, 0);
		begin_actor_record("find_spot", 3, "adjust_attempt", actor_index(g_levinfo));
		put_vec(Extent);
		put_vec(Location);
		printf(" 0 0 -> %d", (int)Fit);
		put_vec(Probe);
		printf(" 6");
		put_actor_rows();
		print_hooks();
		printf("\n");
	}
	far_move_case( 7, "checked", 0 );
	far_move_case( 8, "no_check", 1 );
	g_actors[1] = g_pool[1];
	g_lvl->Actors.ArrayNum = 2;
	A->Class = (UClass*)g_cls_raw[0];
	A->bIsPawn = 0;
	A->bStatic = 0;
	A->bMovable = 1;
	A->bDeleteMe = 0;
	A->bCollideActors = 0;
	A->bCollideWorld = 0;
	A->bBlockActors = 0;
	A->bBlockPlayers = 0;
	A->StandingCount = 1;
	A->Location = FVector(1.f, 2.f, 3.f);
	A->Rotation = FRotator(0, 0, 0);
	A->Region = FPointRegion(g_levinfo);
	A->Base = NULL;
	AActor* Child = g_pool[1];
	Child->Class = (UClass*)g_cls_raw[0];
	Child->bIsPawn = 0;
	Child->bDeleteMe = 0;
	Child->bStatic = 0;
	Child->bMovable = 1;
	Child->bCollideActors = 0;
	Child->bCollideWorld = 0;
	Child->Location = FVector(4.f, 5.f, 6.f);
	Child->Rotation = FRotator(0, 0, 0);
	Child->Region = FPointRegion(g_levinfo);
	Child->Base = A;
	Hit = FCheckResult(7.f);
	Hit.Location = FVector(8.f, 9.f, 10.f);
	Hit.Normal = FVector(11.f, 12.f, 13.f);
	Hit.Item = 14;
	Hit.Actor = NULL;
	clear_hooks();
	FVector BaseDelta(1.f, 0.f, 0.f);
	begin_actor_record("move_actor", 6, "based_move", actor_index(g_levinfo));
	printf(" 0");
	put_vec(BaseDelta);
	printf(" 0 0 0");
	put_hit(Hit);
	printf(" 0 0 0 0 ->");
	Moved = g_lvl->ULevel::MoveActor(A, BaseDelta, A->Rotation, Hit, 0, 0, 0, 0);
	printf(" %d", (int)Moved);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	print_hooks();
	printf("\n");
	g_actors[1] = NULL;
	g_lvl->Actors.ArrayNum = 1;
	A->StandingCount = 0;
	Child->Base = NULL;
	g_actors[0] = g_levinfo;
	g_actor_ops = false;
}

static void move_center_case()
{
	g_actor_ops = true;
	AActor* Actor = g_pool[0];
	set_vptr( Actor, g_actor_vt );
	Actor->Class = (UClass*)g_cls_raw[0];
	Actor->bIsPawn = 0;
	Actor->bDeleteMe = 1;
	Actor->bStatic = 0;
	Actor->bMovable = 1;
	Actor->bCollideActors = 0;
	Actor->bCollideWorld = 1;
	Actor->Location = FVector( 1.f, 2.f, 3.f );
	Actor->Rotation = FRotator( 0, 0, 0 );
	GMem.Top = (BYTE*)g_membuf;
	GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
	g_move_center_mode = true;
	g_trace_calls = 0;
	FCheckResult Hit( 1.f );
	Hit.Location = FVector( 7.f, 8.f, 9.f );
	Hit.Normal = FVector( -7.f, -8.f, -9.f );
	Hit.Item = -3;
	Hit.Actor = NULL;
	FVector Delta( 2.f, -1.f, 3.f );
	clear_hooks();
	begin_actor_record("move_actor", 4, "mvcenter", actor_index(g_levinfo));
	printf(" 0");
	put_vec(Delta);
	printf(" 0 0 0");
	put_hit(Hit);
	printf(" 1 0 0 0 ->");
	UBOOL Result = g_lvl->ULevel::MoveActor( Actor, Delta, Actor->Rotation, Hit, 1, 0, 0, 0 );
	printf(" %d", (int)Result);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	print_hooks();
	printf("\n");
	g_move_center_mode = false;
	g_actor_ops = false;
}

static void move_self_hit_case()
{
	g_actor_ops = true;
	AActor* Actor = g_pool[0];
	set_vptr( Actor, g_actor_vt );
	Actor->Class = (UClass*)g_cls_raw[0];
	Actor->bDeleteMe = 1;
	Actor->bStatic = 0;
	Actor->bMovable = 1;
	Actor->bCollideActors = 1;
	Actor->bCollideWorld = 0;
	Actor->bBlockActors = 1;
	Actor->bBlockPlayers = 0;
	Actor->bIsPawn = 0;
	Actor->Base = NULL;
	Actor->Location = FVector( 1.f, 2.f, 3.f );
	Actor->Rotation = FRotator( 0, 0, 0 );
	GMem.Top = (BYTE*)g_membuf;
	GMem.End = (BYTE*)g_membuf + sizeof g_membuf;
	g_force_self_hit = true;
	FCheckResult Hit( 1.f );
	Hit.Location = FVector( 7.f, 8.f, 9.f );
	Hit.Normal = FVector( -7.f, -8.f, -9.f );
	Hit.Item = -3;
	Hit.Actor = NULL;
	FVector Delta( 1.f, 0.f, 0.f );
	clear_hooks();
	begin_actor_record("move_actor", 5, "self_hit", actor_index(g_levinfo));
	printf(" 0");
	put_vec(Delta);
	printf(" 0 0 0");
	put_hit(Hit);
	printf(" 1 0 0 0 ->");
	UBOOL Result = g_lvl->ULevel::MoveActor( Actor, Delta, Actor->Rotation, Hit, 1, 0, 0, 0 );
	printf(" %d", (int)Result);
	put_hit(Hit);
	printf(" 6");
	put_actor_rows();
	print_hooks();
	printf("\n");
	g_force_self_hit = false;
	g_actor_ops = false;
}


int main() {
	const int N = 2000;
	for (int i = 0; i < N; i++) {
		FVector o(rand_coord(), rand_coord(), rand_coord());
		FVector p(rand_coord(), rand_coord(), rand_coord());
		int ang = rand_angle();
		emit("yaw", o, p, ang, 0, 0, FindDeltaFromRotationYaw(o, p, ang));
		emit("roll", o, p, ang, 0, 0, FindDeltaFromRotationRoll(o, p, ang));
		emit("pitch", o, p, ang, 0, 0, FindDeltaFromRotationPitch(o, p, ang));
		int yaw = rand_angle(), roll = rand_angle(), pitch = rand_angle();
		emit("full", o, p, yaw, roll, pitch, FindDeltaFromRotation(o, p, yaw, roll, pitch));
	}
	init_world();
	check_cases();
	actor_operation_cases();
	move_center_case();
	move_self_hit_case();
}
