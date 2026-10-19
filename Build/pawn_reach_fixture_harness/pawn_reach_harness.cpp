// Scripted-services fixture generator for the APawn reach group (see gen_pawn_reach.sh).
// Links the ORIGINAL UnPawn.o (built from the oracle ref, -O3) and runs actorReachable,
// pointReachable, Reachable, walkReachable, flyReachable, swimReachable, jumpReachable,
// jumpLanding, FindJumpUp, SuggestJumpVelocity and FindBestJump against scripted
// ULevel virtual hooks (MoveActor, FarMoveActor, SingleLineCheck), scripted UModel
// hooks (PointRegion, FastLineCheck) and scripted APawn::findWaterLine.
// UObject::IsA is inline (Class chain walk), so the harness gives each actor a fake
// UClass; the Rust replay answers is_a from the scene's class index.
// Each case prints one line:
//   fn scene... args... | nLog log... | ret loc3 vel3 out3
// scene/args order is fixed by run_case(); the Rust replay reads the same order.
// log entries (first word = kind):
//   0 move:        delta3 rot3 test ignorePawns bIgnoreBases bNoFail | ret time normal3 isGoal newLoc3
//   1 far:         dest3 test bNoCheck | ret
//   2 waterline:   start3 endIn3 endOut3
//   3 singleLine:  end3 start3 traceFlags extent3 nodeFlags | ret time normal3 actorKind
//   4 pointRegion: point3 | zoneIndex
//   5 fastLine:    end3 start3 | ret
// actorKind: 0 null, 1 goal, 2 other. classIndex: 0 APawn, 1 AInventory, 2 ATrigger,
// 3 AWarpZoneMarker, 4 none of those.
// Floats are bit patterns; ints two's complement. No pointer values are printed.
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

// Real Core definitions are not linked; appSqrt matches Core/Src/UnAnsi.cpp.
DOUBLE appSqrt( DOUBLE Value ) { return sqrt( Value ); }
UBOOL GIsEditor = 0;

// Verbatim copy of Core/Src/UnMath.cpp FVector::SafeNormal (Core is not linked; same flags as the engine).
FVector FVector::SafeNormal() const
{
	FLOAT SquareSum = X*X + Y*Y + Z*Z;
	if( SquareSum < SMALL_NUMBER )
		return FVector( 0.f, 0.f, 0.f );

	FLOAT Size = appSqrt(SquareSum);
	FLOAT Scale = 1.f/Size;
	return FVector( X*Scale, Y*Scale, Z*Scale );
}

// Zeroed raw storage only: UClass has no linked constructor; IsA reads SuperField (zero).
alignas(16) static char g_cls_mem[5][sizeof(UClass)];
UClass* AInventory::PrivateStaticClass = (UClass*)g_cls_mem[1];
UClass* ATrigger::PrivateStaticClass = (UClass*)g_cls_mem[2];
UClass* AWarpZoneMarker::PrivateStaticClass = (UClass*)g_cls_mem[3];

static ULevel* g_level;
static UModel* g_model;
static AZoneInfo* g_zone[3];
static APawn* g_pawn;
static AActor* g_other;
static AActor* g_goal_obj;
static AActor* g_goal;

static uint32_t g_log[1 << 14];
static uint32_t g_nlog;

static uint32_t g_scene[128];
static int g_nscene;

// Static native registration from IMPLEMENT_FUNCTION runs before main; no-op here.
BYTE GRegisterNative( INT, const Native& ) { return 0; }

static void lw( uint32_t W ) { if( g_nlog >= sizeof(g_log)/sizeof(g_log[0]) ) abort(); g_log[g_nlog++] = W; }
static void lv( const FVector& V ) { lw( fbits( V.X ) ); lw( fbits( V.Y ) ); lw( fbits( V.Z ) ); }

static void sw( uint32_t W ) { g_scene[g_nscene++] = W; }
static void sf( float F ) { sw( fbits( F ) ); }
static void sv( const FVector& V ) { sf( V.X ); sf( V.Y ); sf( V.Z ); }

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

// Scripted SingleLineCheck (virtual): the hit is drawn from g_svc.
static UBOOL hook_slc( ULevel* Self, FCheckResult& Hit, AActor* Source, const FVector& End, const FVector& Start, DWORD TraceFlags, FVector Extent, BYTE NodeFlags )
{
	lw( 3 );
	lv( End ); lv( Start ); lw( (uint32_t)TraceFlags ); lv( Extent ); lw( (uint32_t)NodeFlags );
	UBOOL Ret = (sm_next( &g_svc ) % 4 != 0) ? 1 : 0;
	FLOAT T = (sm_next( &g_svc ) % 3 == 0) ? 1.0f : svc_unit();
	FVector N( svc_unit() * 2.f - 1.f, svc_unit() * 2.f - 1.f, svc_unit() );
	uint32_t K = (uint32_t)(sm_next( &g_svc ) % 3);
	if( K == 1 && g_goal == NULL )
		K = 2;
	Hit.Time = T;
	Hit.Normal = N;
	Hit.Actor = K == 0 ? NULL : K == 1 ? g_goal : g_other;
	lw( (uint32_t)Ret ); lw( fbits( T ) ); lv( N ); lw( K );
	return Ret;
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

// Scripted UModel::PointRegion: returns one of the three scripted zones.
FPointRegion UModel::PointRegion( AZoneInfo* Zone, FVector Location ) const
{
	lw( 4 );
	lv( Location );
	uint32_t K = (uint32_t)(sm_next( &g_svc ) % 3);
	lw( K );
	FPointRegion R;
	memset( &R, 0, sizeof R );
	R.Zone = g_zone[K];
	return R;
}

// Scripted UModel::FastLineCheck.
BYTE UModel::FastLineCheck( FVector End, FVector Start )
{
	lw( 5 );
	lv( End ); lv( Start );
	BYTE Ret = (BYTE)(sm_next( &g_svc ) % 2);
	lw( Ret );
	return Ret;
}

static void install_level()
{
	auto PmMove = &ULevel::MoveActor;
	auto PmFar = &ULevel::FarMoveActor;
	auto PmSlc = &ULevel::SingleLineCheck;
	g_vt[vt_slot( &PmMove )] = (void*)&hook_move;
	g_vt[vt_slot( &PmFar )] = (void*)&hook_far;
	g_vt[vt_slot( &PmSlc )] = (void*)&hook_slc;
	g_level = (ULevel*)calloc( 1, sizeof(ULevel) );
	g_model = (UModel*)calloc( 1, sizeof(UModel) );
	g_level->Model = g_model;
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

static FVector gen_near( const FVector& C, float R )
{
	return FVector( C.X + gen_range( -R, R ), C.Y + gen_range( -R, R ), C.Z + gen_range( -R, R ) );
}

enum { FN_ACTOR, FN_POINT, FN_REACH, FN_WALK, FN_FLY, FN_SWIM, FN_JUMP, FN_JLAND, FN_FJUMP, FN_SUGGEST, FN_BEST, FN_COUNT };
static const int kCasesPerFn = 200;

static void print_words( const uint32_t* W, int N )
{
	for( int K = 0; K < N; ++K )
		printf( " %08x", W[K] );
}

static void run_case( int F, uint64_t Seed )
{
	g_svc = Seed ^ 0x5EEDF00D0BADC0DEULL;
	g_gen = Seed * 0x2545F4914F6CDD1DULL + 11;
	g_nlog = 0;
	g_nscene = 0;

	// Zones.
	for( int K = 0; K < 3; ++K )
	{
		memset( (void*)g_zone[K], 0, sizeof(AZoneInfo) );
		uint32_t GZ = gen_u( 3 );
		FVector ZG( gen_u( 2 ) ? gen_range( -5.f, 5.f ) : 0.f, gen_u( 2 ) ? gen_range( -5.f, 5.f ) : 0.f,
			GZ == 0 ? -980.f : GZ == 1 ? 980.f : 0.f );
		g_zone[K]->ZoneGravity = ZG;
		g_zone[K]->ZoneFluidFriction = gen_u( 4 ) == 0 ? 1.f : gen_range( 0.f, 2.f );
		g_zone[K]->bWaterZone = gen_u( 2 );
		g_zone[K]->bPainZone = gen_u( 4 ) == 0;
	}

	// Pawn.
	memset( (void*)g_pawn, 0, sizeof(APawn) );
	FVector Loc = gen_vec( -200.f, 200.f, 10 );
	g_pawn->Location = Loc;
	g_pawn->Velocity = gen_vec( -300.f, 300.f, 30 );
	g_pawn->Rotation.Pitch = (INT)gen_u( 65536 ) - 32768;
	g_pawn->Rotation.Yaw = (INT)gen_u( 65536 ) - 32768;
	g_pawn->Rotation.Roll = (INT)gen_u( 65536 ) - 32768;
	g_pawn->CollisionRadius = gen_range( 10.f, 60.f );
	g_pawn->CollisionHeight = gen_range( 20.f, 90.f );
	g_pawn->Physics = (BYTE)gen_u( 6 );
	g_pawn->bCanJump = gen_u( 2 );
	g_pawn->bCanWalk = gen_u( 4 ) != 0;
	g_pawn->bCanSwim = gen_u( 2 );
	g_pawn->bCanFly = gen_u( 3 ) == 0;
	g_pawn->MaxStepHeight = gen_u( 5 ) == 0 ? 0.f : gen_range( 0.f, 60.f );
	g_pawn->GroundSpeed = gen_range( 100.f, 600.f );
	g_pawn->JumpZ = gen_range( 100.f, 600.f );
	g_pawn->MeleeRange = gen_range( 0.f, 100.f );
	g_pawn->BaseEyeHeight = gen_range( 0.f, 60.f );
	g_pawn->XLevel = g_level;
	uint32_t RZ = gen_u( 3 );
	uint32_t FZ = gen_u( 3 );
	uint32_t ZN = gen_u( 4 ); // 0 takes jumpLanding's ZoneNumber==0 early exit.
	g_pawn->Region.ZoneNumber = (BYTE)ZN;
	g_pawn->Region.Zone = g_zone[RZ];
	g_pawn->FootRegion.Zone = g_zone[FZ];
	g_pawn->Class = APawn::PrivateStaticClass;

	// Other actor.
	memset( (void*)g_other, 0, sizeof(APawn) );
	g_other->Location = gen_u( 2 ) ? gen_near( Loc, 80.f ) : gen_vec( -300.f, 300.f, 10 );
	g_other->CollisionRadius = gen_range( 10.f, 60.f );
	g_other->CollisionHeight = gen_range( 20.f, 90.f );
	g_other->bBlockActors = gen_u( 2 );
	uint32_t OtherCls = gen_u( 5 );
	g_other->Class = (UClass*)g_cls_mem[OtherCls];
	uint32_t OtherZone = gen_u( 3 );
	g_other->Region.Zone = g_zone[OtherZone];
	((APawn*)g_other)->FootRegion.Zone = g_zone[OtherZone];

	// Arguments (generated before the goal so the goal can sit near the destination).
	FVector V1 = gen_u( 2 ) ? gen_near( Loc, 120.f ) : gen_vec( -300.f, 300.f, 10 );
	FVector V2 = gen_vec( -400.f, 400.f, 20 );
	FLOAT Thr = gen_u( 8 ) == 0 ? 0.f : gen_range( 0.f, 8.f );
	INT RF = (INT)gen_u( 16 );
	INT Flag = (INT)gen_u( 2 );

	// Goal (AWarpZoneMarker-sized so the markedWarpZone read is valid whatever its class).
	memset( (void*)g_goal_obj, 0, sizeof(APawn) );
	g_goal_obj->Location = gen_u( 2 ) ? gen_near( V1, 40.f ) : gen_vec( -300.f, 300.f, 10 );
	g_goal_obj->CollisionRadius = gen_range( 10.f, 60.f );
	g_goal_obj->CollisionHeight = gen_range( 20.f, 90.f );
	g_goal_obj->bBlockActors = gen_u( 2 );
	int MW = (int)gen_u( 4 ) - 1;
	((AWarpZoneMarker*)g_goal_obj)->markedWarpZone = MW < 0 ? NULL : (AWarpZoneInfo*)g_zone[MW];
	uint32_t GoalCls = gen_u( 5 );
	g_goal_obj->Class = (UClass*)g_cls_mem[GoalCls];
	uint32_t GoalZone = gen_u( 3 );
	g_goal_obj->Region.Zone = g_zone[GoalZone];
	((APawn*)g_goal_obj)->FootRegion.Zone = g_zone[GoalZone];
	UBOOL GoalOn = gen_u( 2 );
	g_goal = GoalOn ? g_goal_obj : NULL;

	GIsEditor = gen_u( 4 ) == 0 ? 1 : 0;

	// Scene record (order mirrored by pawn_reach_fixtures.rs).
	sv( g_pawn->Location ); sv( g_pawn->Velocity );
	sw( (uint32_t)g_pawn->Rotation.Pitch ); sw( (uint32_t)g_pawn->Rotation.Yaw ); sw( (uint32_t)g_pawn->Rotation.Roll );
	sf( g_pawn->CollisionRadius ); sf( g_pawn->CollisionHeight ); sw( g_pawn->Physics );
	sw( g_pawn->bCanJump ); sw( g_pawn->bCanWalk ); sw( g_pawn->bCanSwim ); sw( g_pawn->bCanFly );
	sf( g_pawn->MaxStepHeight ); sf( g_pawn->GroundSpeed ); sf( g_pawn->JumpZ ); sf( g_pawn->MeleeRange ); sf( g_pawn->BaseEyeHeight );
	sw( RZ ); sw( FZ ); sw( ZN );
	for( int K = 0; K < 3; ++K )
	{
		sv( g_zone[K]->ZoneGravity ); sw( g_zone[K]->bWaterZone ); sw( g_zone[K]->bPainZone ); sf( g_zone[K]->ZoneFluidFriction );
	}
	sv( g_other->Location ); sf( g_other->CollisionRadius ); sf( g_other->CollisionHeight ); sw( g_other->bBlockActors ); sw( OtherCls ); sw( OtherZone );
	sw( GoalOn ); sv( g_goal_obj->Location ); sf( g_goal_obj->CollisionRadius ); sf( g_goal_obj->CollisionHeight ); sw( g_goal_obj->bBlockActors );
	sw( (uint32_t)MW ); sw( GoalCls ); sw( GoalZone );
	sw( (uint32_t)GIsEditor );
	// Args.
	sv( V1 ); sv( V2 ); sf( Thr ); sw( (uint32_t)RF ); sw( (uint32_t)Flag );

	// Call the original. Out starts as V2 (Landing / vel in/out).
	FVector Out = V2;
	INT Ret = 0;
	switch( F )
	{
	case FN_ACTOR: Ret = g_pawn->actorReachable( g_other, Flag ); break;
	case FN_POINT: Ret = g_pawn->pointReachable( V1, Flag ); break;
	case FN_REACH: Ret = g_pawn->Reachable( V1, Thr, g_goal ); break;
	case FN_WALK: Ret = g_pawn->walkReachable( V1, Thr, RF, g_goal ); break;
	case FN_FLY: Ret = g_pawn->flyReachable( V1, Thr, RF, g_goal ); break;
	case FN_SWIM: Ret = g_pawn->swimReachable( V1, Thr, RF, g_goal ); break;
	case FN_JUMP: Ret = g_pawn->jumpReachable( V1, Thr, RF, g_goal ); break;
	case FN_JLAND: g_pawn->jumpLanding( V1, Out, Flag ); break;
	case FN_FJUMP: Ret = g_pawn->FindJumpUp( V1, V2, Out, Flag ); break;
	case FN_SUGGEST: g_pawn->SuggestJumpVelocity( V1, Out ); break;
	case FN_BEST: Ret = g_pawn->FindBestJump( V1, V2, Out, Flag ); break;
	default: abort();
	}

	printf( "%08x", (uint32_t)F );
	print_words( g_scene, g_nscene );
	printf( " | %08x", g_nlog );
	print_words( g_log, (int)g_nlog );
	printf( " | %08x", (uint32_t)Ret );
	printf( " %08x %08x %08x", fbits( g_pawn->Location.X ), fbits( g_pawn->Location.Y ), fbits( g_pawn->Location.Z ) );
	printf( " %08x %08x %08x", fbits( g_pawn->Velocity.X ), fbits( g_pawn->Velocity.Y ), fbits( g_pawn->Velocity.Z ) );
	printf( " %08x %08x %08x", fbits( Out.X ), fbits( Out.Y ), fbits( Out.Z ) );
	printf( "\n" );
}

int main()
{
	for( int K = 0; K < 3; ++K )
		g_zone[K] = (AZoneInfo*)calloc( 1, sizeof(AZoneInfo) );
	static_assert( sizeof(APawn) >= sizeof(AWarpZoneMarker), "scripted actors are APawn-sized" );
	g_pawn = (APawn*)calloc( 1, sizeof(APawn) );
	g_other = (AActor*)calloc( 1, sizeof(APawn) );
	g_goal_obj = (AActor*)calloc( 1, sizeof(APawn) );
	install_level();
	// APawn::PrivateStaticClass is defined by UnPawn.o's IMPLEMENT_CLASS; point it at fake storage.
	APawn::PrivateStaticClass = (UClass*)g_cls_mem[0];

	uint64_t Master[FN_COUNT];
	for( int F = 0; F < FN_COUNT; ++F )
		Master[F] = 0x7A3C1E5D9B2F4081ULL ^ ((uint64_t)(F + 1) * 0x100000001B3ULL);
	for( int F = 0; F < FN_COUNT; ++F )
		for( int C = 0; C < kCasesPerFn; ++C )
			run_case( F, sm_next( &Master[F] ) );
	fprintf( stderr, "cases %d\n", FN_COUNT * kCasesPerFn );
	return 0;
}
