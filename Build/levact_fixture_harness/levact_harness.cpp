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
static ALevelInfo* g_levinfo;
static AActor* g_actors[1];
static char g_membuf[1 << 20] __attribute__((aligned(16)));
static char g_cls_raw[7][sizeof(UClass)] __attribute__((aligned(16)));
static char g_obj_raw[7][8192] __attribute__((aligned(16)));

UClass* AActor::PrivateStaticClass = (UClass*)g_cls_raw[0];
UClass* ALevelInfo::PrivateStaticClass = (UClass*)g_cls_raw[1];
UClass* AMover::PrivateStaticClass = (UClass*)g_cls_raw[2];
UClass* AZoneInfo::PrivateStaticClass = (UClass*)g_cls_raw[3];
UClass* ABrush::PrivateStaticClass = (UClass*)g_cls_raw[4];
UClass* AProjectile::PrivateStaticClass = (UClass*)g_cls_raw[5];
UClass* APlayerPawn::PrivateStaticClass = (UClass*)g_cls_raw[6];
FMemStack GMem;

// FMemStack chunk allocation is never reached: the per-case buffer is large enough.
BYTE* FMemStack::AllocateNewChunk( INT MinSize ) { abort(); }

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
	return Head;
}

static FCheckResult* hook_ahlc( FCollisionHashBase* Self, FMemStack& Mem, FVector End, FVector Start, FVector Extent, BYTE Flags )
{
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
	return Head;
}

static UBOOL hook_pc( UModel* Self, FCheckResult& Result, AActor* Owner, FVector Location, FVector Extent, DWORD Flags )
{
	uint64_t h = hvec(hvec(hmix(0x11, 0), Location), Extent);
	if( h % 3 == 0 )
		return 1;
	fill_hit( Result, hmix(h, 0x12) );
	return 0;
}

static UBOOL hook_lc( UModel* Self, FCheckResult& Result, AActor* Owner, FVector End, FVector Start, FVector Extent, DWORD Flags )
{
	uint64_t h = hmix(hvec(hvec(hvec(hmix(0x21, 0), End), Start), Extent), (uint64_t)Flags);
	if( h % 3 == 0 )
		return 1;
	fill_hit( Result, hmix(h, 0x22) );
	return 0;
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

static void init_world()
{
	for( int K = 1; K < 7; K++ )
		((UClass*)g_cls_raw[K])->SuperField = (UClass*)g_cls_raw[0];

	for( int K = 0; K < 6; K++ )
	{
		g_pool[K] = (AActor*)g_obj_raw[K];
		g_pool[K]->Class = (UClass*)g_cls_raw[K == 3 ? 2 : K == 4 ? 3 : K == 5 ? 1 : 0];
	}
	g_levinfo = (ALevelInfo*)g_pool[5];

	auto PmMpc = &ULevel::MultiPointCheck;
	auto PmMlc = &ULevel::MultiLineCheck;
	auto PmAhpc = &FCollisionHashBase::ActorPointCheck;
	auto PmAhlc = &FCollisionHashBase::ActorLineCheck;
	auto PmPc = &UModel::PointCheck;
	auto PmLc = &UModel::LineCheck;
	g_lvl_vt[vt_slot( &PmMpc )] = (void*)&fwd_mpc;
	g_lvl_vt[vt_slot( &PmMlc )] = (void*)&fwd_mlc;
	g_hash_vt[vt_slot( &PmAhpc )] = (void*)&hook_ahpc;
	g_hash_vt[vt_slot( &PmAhlc )] = (void*)&hook_ahlc;
	g_model_vt[vt_slot( &PmPc )] = (void*)&hook_pc;
	g_model_vt[vt_slot( &PmLc )] = (void*)&hook_lc;

	g_lvl = (ULevel*)calloc( 1, sizeof(ULevel) );
	g_model = (UModel*)calloc( 1, sizeof(UModel) );
	g_hash = (FCollisionHashBase*)calloc( 1, sizeof(FCollisionHashBase) + 64 );
	set_vptr( g_lvl, g_lvl_vt );
	set_vptr( g_model, g_model_vt );
	set_vptr( g_hash, g_hash_vt );
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
	return 0;
}
