// Scripted-services fixture generator for APawn::walkMove, flyMove and swimMove (see gen_pawn.sh).
// Links the ORIGINAL UnPawn.o (built from the oracle ref, -O3) and runs those bodies against
// scripted ULevel virtual hooks (MoveActor, FarMoveActor) and a scripted APawn::findWaterLine.
// Each case prints one line of 8-digit hex words:
//   fn | loc3 rot3 maxStep zoneGravity3 waterZone goalMode delta3 threshold bAdjust |
//   nLog | log... | ret finalLoc3
// log entries: kind 0 (move): delta3 rot3 test ignorePawns bIgnoreBases bNoFail ret time normal3 isGoal newLoc3
//              kind 1 (far):  dest3 test bNoCheck ret
//              kind 2 (waterline): start3 endIn3 endOut3
// Floats are bit patterns; ints two's complement.
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

static uint64_t sm_next( uint64_t* S )
{
	uint64_t Z = (*S += 0x9E3779B97F4A7C15ULL);
	Z = (Z ^ (Z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	Z = (Z ^ (Z >> 27)) * 0x94D049BB133111EBULL;
	return Z ^ (Z >> 31);
}

static uint64_t g_svc;
static uint64_t g_gen;

static float unit_of( uint64_t X ) { return (float)(X >> 40) * (1.0f / 16777216.0f); }
static float svc_unit() { return unit_of( sm_next( &g_svc ) ); }
static float gen_range( float Lo, float Hi ) { return Lo + (Hi - Lo) * unit_of( sm_next( &g_gen ) ); }
static uint32_t gen_u( uint32_t N ) { return (uint32_t)(sm_next( &g_gen ) % N); }
static uint32_t fbits( float F ) { uint32_t U; memcpy( &U, &F, sizeof U ); return U; }

static ULevel* g_level;
static AZoneInfo* g_zone;
static APawn* g_pawn;
static AActor* g_goal;
static AActor* g_goal_obj;
static AActor* g_other;

static uint32_t g_log[512];
static uint32_t g_nlog;

// Static native registration from IMPLEMENT_FUNCTION runs before main; no-op here.
BYTE GRegisterNative( INT, const Native& ) { return 0; }

static void lw( uint32_t W ) { g_log[g_nlog++] = W; }
static void lv( const FVector& V ) { lw( fbits( V.X ) ); lw( fbits( V.Y ) ); lw( fbits( V.Z ) ); }

static size_t vt_slot( const void* Pm )
{
	uintptr_t W[2];
	memcpy( W, Pm, sizeof W );
	if( !(W[1] & 1) )
		abort();
	return W[0] / sizeof(void*);
}

static void* g_vt[4096];

// Scripted MoveActor: the result is drawn from g_svc; the actor moves by a scripted fraction.
static UBOOL hook_move( ULevel* Self, AActor* Actor, FVector Delta, FRotator Rot, FCheckResult& Hit, UBOOL Test, UBOOL IgnorePawns, UBOOL bIgnoreBases, UBOOL bNoFail )
{
	lw( 0 );
	lv( Delta );
	lw( (uint32_t)Rot.Pitch ); lw( (uint32_t)Rot.Yaw ); lw( (uint32_t)Rot.Roll );
	lw( (uint32_t)Test ); lw( (uint32_t)IgnorePawns ); lw( (uint32_t)bIgnoreBases ); lw( (uint32_t)bNoFail );
	UBOOL Ret = (sm_next( &g_svc ) % 4 != 0) ? 1 : 0;
	FLOAT T = (sm_next( &g_svc ) % 3 == 0) ? 1.0f : svc_unit();
	FVector N( svc_unit() * 2.f - 1.f, svc_unit() * 2.f - 1.f, svc_unit() );
	UBOOL IsGoal = (g_goal != NULL && sm_next( &g_svc ) % 3 == 0) ? 1 : 0;
	Hit.Time = T;
	Hit.Normal = N;
	Hit.Actor = IsGoal ? g_goal : g_other;
	Actor->Location = Actor->Location + Delta * T;
	lw( (uint32_t)Ret ); lw( fbits( T ) ); lv( N ); lw( (uint32_t)IsGoal ); lv( Actor->Location );
	return Ret;
}

// Scripted FarMoveActor: teleports the actor to Dest.
static UBOOL hook_far( ULevel* Self, AActor* Actor, FVector Dest, UBOOL Test, UBOOL bNoCheck )
{
	lw( 1 );
	lv( Dest );
	lw( (uint32_t)Test ); lw( (uint32_t)bNoCheck );
	Actor->Location = Dest;
	lw( 1 );
	return 1;
}

// Scripted APawn::findWaterLine: End is replaced by a scripted point near Start.
void APawn::findWaterLine( FVector Start, FVector& End )
{
	lw( 2 );
	lv( Start );
	lv( End );
	if( sm_next( &g_svc ) % 2 == 0 )
		End = Start;
	else
		End = FVector( Start.X + svc_unit() * 8.f, Start.Y + svc_unit() * 8.f, Start.Z - svc_unit() * 16.f );
	lv( End );
}

static void install_level()
{
	auto PmMove = &ULevel::MoveActor;
	auto PmFar = &ULevel::FarMoveActor;
	g_vt[vt_slot( &PmMove )] = (void*)&hook_move;
	g_vt[vt_slot( &PmFar )] = (void*)&hook_far;
	g_level = (ULevel*)calloc( 1, sizeof(ULevel) );
	void** Vt = g_vt;
	memcpy( (void*)g_level, &Vt, sizeof Vt );
}

static FVector gen_vec( float Lo, float Hi, uint32_t ZeroPct )
{
	float C[3];
	for( int K = 0; K < 3; ++K )
		C[K] = (ZeroPct && gen_u( 100 ) < ZeroPct) ? 0.f : gen_range( Lo, Hi );
	return FVector( C[0], C[1], C[2] );
}

enum { FN_WALK, FN_FLY, FN_SWIM, FN_COUNT };
static const int kCasesPerFn = 800;

static void print_words( const uint32_t* W, int N )
{
	for( int K = 0; K < N; ++K )
		printf( " %08x", W[K] );
}

static void run_case( int F, uint64_t Seed )
{
	g_svc = Seed ^ 0xA5A5F00D12345678ULL;
	g_gen = Seed * 0x2545F4914F6CDD1DULL + 7;
	g_nlog = 0;

	memset( (void*)g_pawn, 0, sizeof(APawn) );
	g_pawn->Location = gen_vec( -200.f, 200.f, 10 );
	g_pawn->Rotation.Pitch = (INT)gen_u( 65536 ) - 32768;
	g_pawn->Rotation.Yaw = (INT)gen_u( 65536 ) - 32768;
	g_pawn->Rotation.Roll = (INT)gen_u( 65536 ) - 32768;
	g_pawn->MaxStepHeight = gen_u( 5 ) == 0 ? 0.f : gen_range( 0.f, 60.f );
	g_pawn->XLevel = g_level;
	g_pawn->Region.Zone = g_zone;

	uint32_t GZ = gen_u( 3 );
	FVector ZG( gen_u( 2 ) ? gen_range( -5.f, 5.f ) : 0.f, gen_u( 2 ) ? gen_range( -5.f, 5.f ) : 0.f,
		GZ == 0 ? -980.f : GZ == 1 ? 980.f : 0.f );
	g_zone->ZoneGravity = ZG;
	g_zone->bWaterZone = (gen_u( 2 ) == 0) ? 1 : 0;

	UBOOL Goal = gen_u( 2 );
	g_goal = Goal ? g_goal_obj : NULL;
	FVector Delta = gen_vec( -50.f, 50.f, 15 );
	FLOAT Thr = gen_u( 8 ) == 0 ? 0.f : gen_range( 0.f, 8.f );
	INT Adj = gen_u( 2 );

	uint32_t In[32];
	int NIn = 0;
	In[NIn++] = fbits( g_pawn->Location.X ); In[NIn++] = fbits( g_pawn->Location.Y ); In[NIn++] = fbits( g_pawn->Location.Z );
	In[NIn++] = (uint32_t)g_pawn->Rotation.Pitch; In[NIn++] = (uint32_t)g_pawn->Rotation.Yaw; In[NIn++] = (uint32_t)g_pawn->Rotation.Roll;
	In[NIn++] = fbits( g_pawn->MaxStepHeight );
	In[NIn++] = fbits( ZG.X ); In[NIn++] = fbits( ZG.Y ); In[NIn++] = fbits( ZG.Z );
	In[NIn++] = (uint32_t)g_zone->bWaterZone;
	In[NIn++] = (uint32_t)Goal;
	In[NIn++] = fbits( Delta.X ); In[NIn++] = fbits( Delta.Y ); In[NIn++] = fbits( Delta.Z );
	In[NIn++] = fbits( Thr );
	In[NIn++] = (uint32_t)Adj;

	INT Ret;
	FCheckResult Hit( 1.f );
	if( F == FN_WALK )
		Ret = g_pawn->walkMove( Delta, Hit, g_goal, Thr, Adj );
	else if( F == FN_FLY )
		Ret = g_pawn->flyMove( Delta, g_goal, Thr, Adj );
	else
		Ret = g_pawn->swimMove( Delta, g_goal, Thr, Adj );

	printf( "%08x", (uint32_t)F );
	print_words( In, NIn );
	printf( " | %08x", g_nlog );
	print_words( g_log, (int)g_nlog );
	printf( " | %08x", (uint32_t)Ret );
	printf( " %08x %08x %08x", fbits( g_pawn->Location.X ), fbits( g_pawn->Location.Y ), fbits( g_pawn->Location.Z ) );
	printf( "\n" );
}

int main()
{
	g_zone = (AZoneInfo*)calloc( 1, sizeof(AZoneInfo) );
	g_pawn = (APawn*)calloc( 1, sizeof(APawn) );
	g_goal_obj = (AActor*)calloc( 1, sizeof(AActor) );
	g_other = (AActor*)calloc( 1, sizeof(AActor) );
	install_level();

	uint64_t Master[FN_COUNT];
	for( int F = 0; F < FN_COUNT; ++F )
		Master[F] = 0x1F2E3D4C5B6A7988ULL ^ ((uint64_t)(F + 1) * 0x100000001B3ULL);
	for( int F = 0; F < FN_COUNT; ++F )
		for( int C = 0; C < kCasesPerFn; ++C )
			run_case( F, sm_next( &Master[F] ) );
	fprintf( stderr, "cases %d\n", FN_COUNT * kCasesPerFn );
	return 0;
}
