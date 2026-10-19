// Scripted-services fixture generator for the physics slice (see gen_physic.sh).
// Links the original UnPhysic.o built from the pinned ref 58a73d4 (-O3, real flags)
// and runs its APawn/AActor bodies against scripted engine hooks. Each case prints
// one line:
//   <fn> <seed16> <input words>... | <output words>...
// Words are 8-digit lowercase hex (floats as bit patterns, ints two's complement).
// Built with -fno-access-control; layout is unchanged, the original object is linked.
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cmath>
#include "EnginePrivate.h"
#include "UnMesh.h"

// ---- deterministic streams -------------------------------------------------

static uint64_t sm_next( uint64_t* S )
{
	uint64_t Z = (*S += 0x9E3779B97F4A7C15ULL);
	Z = (Z ^ (Z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	Z = (Z ^ (Z >> 27)) * 0x94D049BB133111EBULL;
	return Z ^ (Z >> 31);
}

static uint64_t g_svc;   // service draws, consumed by hooks in call order
static uint64_t g_gen;   // input generation (harness only)

static float unit_of( uint64_t X ) { return (float)(X >> 40) * (1.0f / 16777216.0f); }
static float svc_unit() { return unit_of( sm_next( &g_svc ) ); }
static float svc_sym() { return 2.0f * svc_unit() - 1.0f; }

static uint32_t fbits( float F ) { uint32_t U; memcpy( &U, &F, sizeof U ); return U; }
static float fval( uint32_t U ) { float F; memcpy( &F, &U, sizeof F ); return F; }

static uint32_t gen_u( uint32_t N ) { return (uint32_t)(sm_next( &g_gen ) % N); }
static float gen_range( float Lo, float Hi ) { return Lo + (Hi - Lo) * unit_of( sm_next( &g_gen ) ); }
static uint32_t gen_bits( float Lo, float Hi ) { return fbits( gen_range( Lo, Hi ) ); }
// Writes 3 float words; each component is exactly zero with probability ZeroPct%.
static void gen_vec( uint32_t* W, float Lo, float Hi, uint32_t ZeroPct )
{
	for( int K = 0; K < 3; ++K )
		W[K] = (ZeroPct && gen_u( 100 ) < ZeroPct) ? 0u : gen_bits( Lo, Hi );
}

// ---- log ----------------------------------------------------------------------

static char g_log[1 << 16];
static size_t g_loglen;

static void lg( const char* Fmt, ... )
{
	va_list Ap;
	va_start( Ap, Fmt );
	int N = vsnprintf( g_log + g_loglen, sizeof g_log - g_loglen, Fmt, Ap );
	va_end( Ap );
	if( N < 0 || g_loglen + (size_t)N >= sizeof g_log )
		abort();
	g_loglen += (size_t)N;
}

static uint64_t fnv1a( const char* P, size_t N )
{
	uint64_t H = 1469598103934665603ULL;
	for( size_t K = 0; K < N; ++K )
	{
		H ^= (unsigned char)P[K];
		H *= 1099511628211ULL;
	}
	return H;
}

class HarnessOutputDevice : public FOutputDevice
{
public:
	void Serialize( const TCHAR*, EName ) { lg( "debugf\n" ); }
};

static HarnessOutputDevice g_harness_log;
FOutputDevice* GLog = &g_harness_log;

void FOutputDevice::Logf( const TCHAR*, ... )
{
	lg( "debugf\n" );
}

// ---- objects ------------------------------------------------------------------

static char g_subj_mem[8192] __attribute__((aligned(16)));
static char g_zone_mem[8192] __attribute__((aligned(16)));
static char g_pool_mem[3][8192] __attribute__((aligned(16)));
static char g_cls_raw[5][sizeof(UClass)] __attribute__((aligned(16)));

typedef char g_size_apawn[sizeof(APawn) <= 8192 ? 1 : -1];
typedef char g_size_zone[sizeof(AZoneInfo) <= 8192 ? 1 : -1];
typedef char g_size_fname[sizeof(FName) == sizeof(int32_t) ? 1 : -1];

// Class codes: 0 AActor, 1 APawn, 2 APlayerPawn, 3 ADecoration, 4 ACarcass.
UClass* AActor::PrivateStaticClass = (UClass*)g_cls_raw[0];
UClass* APawn::PrivateStaticClass = (UClass*)g_cls_raw[1];
UClass* APlayerPawn::PrivateStaticClass = (UClass*)g_cls_raw[2];
UClass* ADecoration::PrivateStaticClass = (UClass*)g_cls_raw[3];
UClass* ACarcass::PrivateStaticClass = (UClass*)g_cls_raw[4];

static UClass* g_cls[5];
static AActor* g_pool[3];
static AZoneInfo* g_zone;
static ULevel* g_level;
static FBspSurf* g_surf[2];  // [0] plain, [1] special

static int pidx( const void* P )
{
	for( int K = 0; K < 3; ++K )
		if( g_pool[K] == P )
			return K;
	return -1;
}

static uint32_t class_code( const AActor* A )
{
	for( uint32_t K = 0; K < 5; ++K )
		if( A->Class == g_cls[K] )
			return K;
	return 0xffffffffu;
}

// ---- scripted hooks -------------------------------------------------------------

UBOOL UObject::IsProbing( FName ProbeName )
{
	UBOOL Ret = (UBOOL)(sm_next( &g_svc ) & 1);
	lg( "is_probing %d\n", Ret );
	return Ret;
}

FBspSurf* FCheckResult::BspSurf( FLOAT Radius ) const
{
	uint64_t D = sm_next( &g_svc ) % 3;
	FBspSurf* Surf = (D == 0) ? g_surf[1] : (D == 1) ? g_surf[0] : NULL;
	lg( "bsp_surf %08x %d\n", fbits( Radius ), (D == 0) ? 2 : (D == 1) ? 1 : 0 );
	return Surf;
}

FVector AActor::GetCylinderExtent() const
{
	FVector V( CollisionRadius, CollisionRadius, CollisionHeight );
	lg( "cyl_extent %08x %08x %08x\n", fbits( V.X ), fbits( V.Y ), fbits( V.Z ) );
	return V;
}

void AActor::SetBase( AActor* NewBase, int bNotifyActor )
{
	lg( "set_base %d\n", pidx( NewBase ) );
}

static int g_tag_mount, g_tag_hitwall, g_tag_landed, g_tag_fell, g_tag_falling, g_tag_may_fall, g_tag_do_jump;
static int g_tag_update_camera, g_tag_interpolate_end, g_tag_finished_interpolation;

UFunction* UObject::FindFunctionChecked( FName InName, UBOOL Global )
{
	int32_t Idx, M, H, L, Fo, Falling, MayFall, DoJump, UpdateCamera, InterpolateEnd, FinishedInterpolation;
	memcpy( &Idx, &InName, sizeof Idx );
	memcpy( &M, &ENGINE_Mount, sizeof M );
	memcpy( &H, &ENGINE_HitWall, sizeof H );
	memcpy( &L, &ENGINE_Landed, sizeof L );
	memcpy( &Fo, &ENGINE_FellOutOfWorld, sizeof Fo );
	memcpy( &Falling, &ENGINE_Falling, sizeof Falling );
	memcpy( &MayFall, &ENGINE_MayFall, sizeof MayFall );
	memcpy( &DoJump, &ENGINE_DoJump, sizeof DoJump );
	memcpy( &UpdateCamera, &ENGINE_UpdateCamera, sizeof UpdateCamera );
	memcpy( &InterpolateEnd, &ENGINE_InterpolateEnd, sizeof InterpolateEnd );
	memcpy( &FinishedInterpolation, &ENGINE_FinishedInterpolation, sizeof FinishedInterpolation );
	if( Idx == M ) return (UFunction*)&g_tag_mount;
	if( Idx == H ) return (UFunction*)&g_tag_hitwall;
	if( Idx == L ) return (UFunction*)&g_tag_landed;
	if( Idx == Fo ) return (UFunction*)&g_tag_fell;
	if( Idx == Falling ) return (UFunction*)&g_tag_falling;
	if( Idx == MayFall ) return (UFunction*)&g_tag_may_fall;
	if( Idx == DoJump ) return (UFunction*)&g_tag_do_jump;
	if( Idx == UpdateCamera ) return (UFunction*)&g_tag_update_camera;
	if( Idx == InterpolateEnd ) return (UFunction*)&g_tag_interpolate_end;
	if( Idx == FinishedInterpolation ) return (UFunction*)&g_tag_finished_interpolation;
	abort();
}

static void process_event_body( UObject* Receiver, UFunction* Function, void* Parms, void* Result )
{
	if( Function == (UFunction*)&g_tag_mount )
	{
		APawn_eventMount_Parms* P = (APawn_eventMount_Parms*)Parms;
		lg( "event_mount %08x %08x %08x\n", fbits( P->Delta.X ), fbits( P->Delta.Y ), fbits( P->Delta.Z ) );
	}
	else if( Function == (UFunction*)&g_tag_hitwall )
	{
		AActor_eventHitWall_Parms* P = (AActor_eventHitWall_Parms*)Parms;
		lg( "event_hit_wall %08x %08x %08x %d\n", fbits( P->HitNormal.X ), fbits( P->HitNormal.Y ), fbits( P->HitNormal.Z ), pidx( P->HitWall ) );
	}
	else if( Function == (UFunction*)&g_tag_landed )
	{
		AActor_eventLanded_Parms* P = (AActor_eventLanded_Parms*)Parms;
		lg( "event_landed %08x %08x %08x\n", fbits( P->HitNormal.X ), fbits( P->HitNormal.Y ), fbits( P->HitNormal.Z ) );
	}
	else if( Function == (UFunction*)&g_tag_fell )
		lg( "event_fell_out_of_world\n" );
	else if( Function == (UFunction*)&g_tag_falling )
		lg( "event_falling\n" );
	else if( Function == (UFunction*)&g_tag_may_fall )
		lg( "event_may_fall\n" );
	else if( Function == (UFunction*)&g_tag_do_jump )
		lg( "event_do_jump %08x\n", fbits( ((APlayerPawn_eventDoJump_Parms*)Parms)->F ) );
	else if( Function == (UFunction*)&g_tag_update_camera )
		lg( "event_update_camera %08x\n", fbits( ((AInterpolationManager_eventUpdateCamera_Parms*)Parms)->Pct ) );
	else if( Function == (UFunction*)&g_tag_interpolate_end )
	{
		AInterpolationPoint_eventInterpolateEnd_Parms* P = (AInterpolationPoint_eventInterpolateEnd_Parms*)Parms;
		lg( "event_interpolate_end %d %d %d\n", pidx( (AActor*)Receiver ), pidx( (AActor*)P->Other ), P->bForward );
	}
	else if( Function == (UFunction*)&g_tag_finished_interpolation )
	{
		AActor_eventFinishedInterpolation_Parms* P = (AActor_eventFinishedInterpolation_Parms*)Parms;
		lg( "event_finished_interpolation %d %d\n", pidx( (AActor*)Receiver ), pidx( (AActor*)P->Other ) );
	}
	else
		abort();
}

// Vtable entry for objects with g_obj_vt; body is shared with the member override.
static void process_event_fn( AActor* Self, UFunction* F, void* P, void* R ) { process_event_body( Self, F, P, R ); }
void AActor::ProcessEvent( UFunction* F, void* P, void* R ) { process_event_body( this, F, P, R ); }

FLOAT appFrand()
{
	FLOAT U = svc_unit();
	lg( "app_frand %08x\n", fbits( U ) );
	return U;
}

DOUBLE appSqrt( DOUBLE Value )
{
	return sqrt( Value );
}
DOUBLE appAtan2( DOUBLE Y, DOUBLE X )
{
	return atan2( Y, X );
}

FRotator FVector::Rotation()
{
	FRotator R;
	R.Yaw = (INT)(appAtan2(Y,X) * (FLOAT)MAXWORD / (2.f*PI));
	R.Pitch = (INT)(appAtan2(Z,appSqrt(X*X+Y*Y)) * (FLOAT)MAXWORD / (2.f*PI));
	R.Roll = 0;
	return R;
}

// Copied from UnMath.cpp (FVector::SafeNormal) so the harness builds the same
// expression shape as the engine object.
FVector FVector::SafeNormal() const
{
	FLOAT SquareSum = X*X + Y*Y + Z*Z;
	if( SquareSum < SMALL_NUMBER )
		return FVector( 0.f, 0.f, 0.f );

	FLOAT Size = appSqrt(SquareSum);
	FLOAT Scale = 1.f/Size;
	return FVector( X*Scale, Y*Scale, Z*Scale );
}

// Target of the WORK-copy shim in UnMath.h FRotator::Vector().
FVector hp2_harness_rotation_vector( const FRotator& Rot )
{
	FLOAT X = svc_sym();
	FLOAT Y = svc_sym();
	FLOAT Z = svc_sym();
	lg( "rot_vector %08x %08x %08x\n", fbits( X ), fbits( Y ), fbits( Z ) );
	return FVector( X, Y, Z );
}


// Scripted hit: time, normal, location, actor. Draw order is fixed.
static void scripted_hit( FCheckResult& Hit )
{
	FLOAT T = svc_unit();
	FLOAT Nx = svc_sym();
	FLOAT Ny = svc_sym();
	FLOAT Nz = svc_sym();
	FLOAT Lx = svc_unit();
	FLOAT Ly = svc_unit();
	FLOAT Lz = svc_unit();
	uint64_t X = sm_next( &g_svc );
	if( X % 10 == 0 )
		T = 1.f;  // about 1 in 10 scripted hits is a no-hit (Time == 1)
	// Engine invariant: a hit with Time < 1 always has an actor (step_up/hit_wall dereference it unguarded).
	AActor* A = (X % 4 == 0 && T >= 1.f) ? NULL : g_pool[(X >> 8) % 3];
	Hit.Time = T;
	Hit.Normal = FVector( Nx, Ny, Nz );
	Hit.Location = FVector( Lx, Ly, Lz );
	Hit.Actor = A;
	lg( "hit %08x %08x %08x %08x %08x %08x %08x %d\n", fbits( T ), fbits( Nx ), fbits( Ny ), fbits( Nz ), fbits( Lx ), fbits( Ly ), fbits( Lz ), pidx( A ) );
}

static int g_walk_mode;
static int g_walk_move_calls;
static int g_walk_slc_calls;
// ULevel virtual hooks. Slots are filled from the member-pointer offsets.
static void* g_vt[4096];
static void* g_obj_vt[4096];  // vptr target for subject/pool/zone objects (ProcessEvent slot only)
static void* g_mesh_vt[4096];
static FCoords g_bone_coords;


// Scripted AActor::MoveActor results for family "smooth" (g_walk_mode 7).
struct SmoothStep { int Ret; float Time; FVector Normal; int Actor; };
static SmoothStep g_smooth_plan[3];

static UBOOL hook_move_actor( ULevel* Self, AActor* Actor, FVector Delta, FRotator NewRot, FCheckResult& Hit, UBOOL Test, UBOOL IgnorePawns, UBOOL bIgnoreBases, UBOOL bNoFail )
{
	UBOOL Ret;
	if( g_walk_mode == 7 )
	{
		const int Call = g_walk_move_calls++;
		const SmoothStep P = Call < 3 ? g_smooth_plan[Call] : SmoothStep{ 1, 1.f, FVector( 0.f, 0.f, 0.f ), -1 };
		Ret = P.Ret;
		Hit.Time = P.Ret ? 1.f : P.Time;
		Hit.Normal = P.Ret ? FVector( 0.f, 0.f, 0.f ) : P.Normal;
		Hit.Location = FVector( 0.f, 0.f, 0.f );
		Hit.Actor = (P.Ret || P.Actor < 0) ? NULL : g_pool[P.Actor];
	}
	else if( g_walk_mode == 1 || g_walk_mode == 2 )
		Ret = (g_walk_move_calls++ < 2) ? 1 : 0;
	else if( g_walk_mode == 3 || g_walk_mode == 6 )
		Ret = (g_walk_move_calls++ < 3) ? 1 : 0;
	else if( g_walk_mode == 4 )
	{
		int Call = g_walk_move_calls++;
		Ret = Call != 0;
		if( Call == 0 )
		{
			Hit.Time = 0.5f;
			Hit.Normal = FVector( 0.f, 0.f, 0.8f );
			Hit.Location = FVector( 0.f, 0.f, 0.f );
			Hit.Actor = g_pool[0];
		}
		else if( Call <= 3 )
		{
			Hit.Time = 1.f;
			Hit.Normal = FVector( 0.f, 0.f, 0.f );
			Hit.Location = FVector( 0.f, 0.f, 0.f );
			Hit.Actor = NULL;
		}
	}
	else if( g_walk_mode == 5 )
	{
		++g_walk_move_calls;
		Actor->Physics = PHYS_Swimming;
		Ret = 1;
	}
	else
		Ret = (sm_next( &g_svc ) % 4 == 0) ? 1 : 0;
	if( Ret && (g_walk_mode == 1 || g_walk_mode == 2 || g_walk_mode == 3 || g_walk_mode == 5 || g_walk_mode == 6) )
	{
		Hit.Time = 1.f;
		Hit.Normal = FVector( 0.f, 0.f, 0.f );
		Hit.Location = FVector( 0.f, 0.f, 0.f );
		Hit.Actor = NULL;
	}
	lg( "move %08x %08x %08x %d %d %d %d\n", fbits( Delta.X ), fbits( Delta.Y ), fbits( Delta.Z ), NewRot.Pitch, NewRot.Yaw, NewRot.Roll, Ret );
	if( !Ret && g_walk_mode != 4 && g_walk_mode != 7 )
		scripted_hit( Hit );
	return Ret;
}

static UBOOL hook_far_move_actor( ULevel* Self, AActor* Actor, FVector Dest, UBOOL Test, UBOOL bNoCheck )
{
	lg( "far %08x %08x %08x %d %d\n", fbits( Dest.X ), fbits( Dest.Y ), fbits( Dest.Z ), Test, bNoCheck );
	Actor->Location = Dest;
	return 1;
}

static UBOOL hook_slc( ULevel* Self, FCheckResult& Hit, AActor* Src, const FVector& End, const FVector& Start, DWORD TraceFlags, FVector Extent, BYTE NodeFlags )
{
	UBOOL Ret;
	UBOOL Scripted = 0;
	if( g_walk_mode == 1 )
	{
		int Call = g_walk_slc_calls++;
		if( Call == 0 )
		{
			Scripted = 1;
			Ret = 1;
			Hit.Time = 2.f/3.f;
			Hit.Normal = FVector( 0.f, 0.f, 0.9f );
			Hit.Location = FVector( 0.f, 0.f, 0.f );
			Hit.Actor = NULL;
		}
		else
			Ret = (sm_next( &g_svc ) % 3 == 0) ? 1 : 0;
	}
	else if( g_walk_mode == 2 || g_walk_mode == 3 || g_walk_mode == 4 || g_walk_mode == 6 )
	{
		int Call = g_walk_slc_calls++;
		Scripted = 1;
		int HitCall = (g_walk_mode == 2) ? 1 : (g_walk_mode == 3 || g_walk_mode == 6) ? 3 : -1;
		int FloorCall = (g_walk_mode == 2) ? 22 : (g_walk_mode == 3 ? 7 : (g_walk_mode == 6 ? 4 : 0));
		if( Call == HitCall )
		{
			Ret = 0;
			Hit.Time = 0.5f;
			Hit.Normal = (g_walk_mode == 2) ? FVector( 1.f, 0.f, 0.f ) : FVector( 0.8f, 0.6f, 0.f );
			Hit.Location = FVector( 0.f, 0.f, 0.f );
			Hit.Actor = NULL;
		}
		else if( Call == FloorCall )
		{
			Ret = 1;
			Hit.Time = 2.f/3.f;
			Hit.Normal = FVector( 0.f, 0.f, 0.9f );
			Hit.Location = FVector( 0.f, 0.f, 0.f );
			Hit.Actor = NULL;
		}
		else
		{
			Ret = 1;
			Hit.Time = 1.f;
			Hit.Normal = FVector( 0.f, 0.f, 0.f );
			Hit.Location = FVector( 0.f, 0.f, 0.f );
			Hit.Actor = NULL;
		}
	}
	else
		Ret = (sm_next( &g_svc ) % 3 == 0) ? 1 : 0;
	lg( "slc %d %08x %08x %08x %08x %08x %08x %08x %08x %08x %d\n", (TraceFlags == TRACE_AllBlocking) ? 1 : 0,
		fbits( End.X ), fbits( End.Y ), fbits( End.Z ), fbits( Start.X ), fbits( Start.Y ), fbits( Start.Z ),
		fbits( Extent.X ), fbits( Extent.Y ), fbits( Extent.Z ), Ret );
	lg( "slc_source %d %d\n", pidx( Src ), Src == NULL );
	if( !Ret && !Scripted )
		scripted_hit( Hit );
	return Ret;
}


// Vtable slot of a virtual member function pointer (ARM member-pointer ABI, see above).
static size_t vt_slot( const void* Pm )
{
	uintptr_t W[2];
	memcpy( W, Pm, sizeof W );
	if( !(W[1] & 1) )
		abort();
	return W[0] / sizeof(void*);
}
static FCoords hook_get_bone_coords( const UMesh* Self, AActor* Owner, INT Bone )
{
	return g_bone_coords;
}


static void install_level()
{
	// Apple arm64 uses the ARM member-pointer ABI: word0 is the exact vtable byte
	// offset and word1 bit0 marks a virtual member (checked by vt_slot).
	auto PmMove = &ULevel::MoveActor;
	auto PmFar = &ULevel::FarMoveActor;
	auto PmSlc = &ULevel::SingleLineCheck;
	auto PmPe = &UObject::ProcessEvent;
	g_vt[vt_slot( &PmMove )] = (void*)&hook_move_actor;
	g_vt[vt_slot( &PmFar )] = (void*)&hook_far_move_actor;
	g_vt[vt_slot( &PmSlc )] = (void*)&hook_slc;
	auto PmBone = &UMesh::GetBoneCoords;
	g_mesh_vt[vt_slot( &PmBone )] = (void*)&hook_get_bone_coords;
	g_obj_vt[vt_slot( &PmPe )] = (void*)&process_event_fn;
	g_level = (ULevel*)calloc( 1, sizeof(ULevel) );
	void** Vt = g_vt;
	memcpy( (void*)g_level, &Vt, sizeof Vt );
}
// ---- word layout ----------------------------------------------------------------
// input: subject 0..31, zone 32..38, pool 39..47, hit 48..55, args 56..
// output: subject 0..31, zone 32..38, hit 39..46, ret 47, digest hi/lo 48..49, log length 50.

enum { SUBJ_W = 32, ZONE_W = 7, POOL_OFF = 39, HIT_OFF = 48, ARG0 = 56, OUT_W = 51, IN_MAX = 80 };
enum { FN_CALC, FN_MOUNT, FN_STEP, FN_HITWALL, FN_LANDED, FN_COUNT };
static const char* const kNames[FN_COUNT] = { "calc_velocity", "mount", "step_up", "process_hit_wall", "process_landed" };
static const int kArgW[FN_COUNT] = { 9, 3, 9, 4, 6 };
static const int kCasesPerFn = 1200;

static FVector vec3( const uint32_t* W ) { return FVector( fval( W[0] ), fval( W[1] ), fval( W[2] ) ); }
static void put3( uint32_t* W, const FVector& V ) { W[0] = fbits( V.X ); W[1] = fbits( V.Y ); W[2] = fbits( V.Z ); }

static void load_subject( AActor* A, const uint32_t* W )
{
	APawn* P = (APawn*)A;
	ADecoration* D = (ADecoration*)A;
	ACarcass* C = (ACarcass*)A;
	A->Location = vec3( W );
	A->Rotation.Pitch = (INT)W[3];
	A->Rotation.Yaw = (INT)W[4];
	A->Rotation.Roll = (INT)W[5];
	A->Velocity = vec3( W + 6 );
	A->Acceleration = vec3( W + 9 );
	A->Physics = (BYTE)W[12];
	A->bIsPawn = W[13] != 0;
	A->bJustTeleported = W[14] != 0;
	P->bIsWalking = W[15] != 0;
	P->bFromWall = W[16] != 0;
	P->Destination = vec3( W + 17 );
	P->AccelRate = fval( W[20] );
	P->DesiredSpeed = fval( W[21] );
	P->MaxStepHeight = fval( W[22] );
	P->MaxMountHeight = fval( W[23] );
	P->MinHitWall = fval( W[24] );
	A->CollisionRadius = fval( W[25] );
	A->CollisionHeight = fval( W[26] );
	A->Mass = fval( W[27] );
	A->Buoyancy = fval( W[28] );
	D->numLandings = (INT)W[29];
	C->bSlidingCarcass = W[30] != 0;
	A->Region.Zone = g_zone;
	A->XLevel = g_level;
	A->Class = g_cls[W[31]];
}

static void store_subject( const AActor* A, uint32_t* W )
{
	const APawn* P = (const APawn*)A;
	const ADecoration* D = (const ADecoration*)A;
	const ACarcass* C = (const ACarcass*)A;
	put3( W, A->Location );
	W[3] = (uint32_t)A->Rotation.Pitch;
	W[4] = (uint32_t)A->Rotation.Yaw;
	W[5] = (uint32_t)A->Rotation.Roll;
	put3( W + 6, A->Velocity );
	put3( W + 9, A->Acceleration );
	W[12] = (uint32_t)A->Physics;
	W[13] = (uint32_t)A->bIsPawn;
	W[14] = (uint32_t)A->bJustTeleported;
	W[15] = (uint32_t)P->bIsWalking;
	W[16] = (uint32_t)P->bFromWall;
	put3( W + 17, P->Destination );
	W[20] = fbits( P->AccelRate );
	W[21] = fbits( P->DesiredSpeed );
	W[22] = fbits( P->MaxStepHeight );
	W[23] = fbits( P->MaxMountHeight );
	W[24] = fbits( P->MinHitWall );
	W[25] = fbits( A->CollisionRadius );
	W[26] = fbits( A->CollisionHeight );
	W[27] = fbits( A->Mass );
	W[28] = fbits( A->Buoyancy );
	W[29] = (uint32_t)D->numLandings;
	W[30] = (uint32_t)C->bSlidingCarcass;
	W[31] = class_code( A );
}

static void load_zone( AZoneInfo* Z, const uint32_t* W )
{
	Z->ZoneGravity = vec3( W );
	Z->ZoneVelocity = vec3( W + 3 );
	Z->bBounceVelocity = W[6] != 0;
}

static void store_zone( const AZoneInfo* Z, uint32_t* W )
{
	put3( W, Z->ZoneGravity );
	put3( W + 3, Z->ZoneVelocity );
	W[6] = (uint32_t)Z->bBounceVelocity;
}

static void load_pool( AActor* A, const uint32_t* W )
{
	A->Class = g_cls[W[0]];
	A->Mass = fval( W[1] );
	((ADecoration*)A)->bPushable = W[2] != 0;
}

static void store_hit( const FCheckResult& H, uint32_t* W )
{
	W[0] = fbits( H.Time );
	put3( W + 1, H.Normal );
	put3( W + 4, H.Location );
	W[7] = (uint32_t)pidx( H.Actor );
}

static AActor* pool_or_null( uint32_t Idx )
{
	int32_t I = (int32_t)Idx;
	return (I < 0) ? NULL : g_pool[I];
}

// ---- input generation ---------------------------------------------------------------

static void gen_subject( uint32_t* W, uint32_t ClsLo, uint32_t ClsN )
{
	gen_vec( W, -200.f, 200.f, 0 );
	for( int K = 0; K < 3; ++K )
		W[3 + K] = (uint32_t)((int32_t)gen_u( 131072 ) - 65536);
	gen_vec( W + 6, -600.f, 600.f, 15 );
	gen_vec( W + 9, -400.f, 400.f, 30 );
	W[12] = gen_u( 10 );
	W[13] = gen_u( 2 );
	W[14] = gen_u( 2 );
	W[15] = gen_u( 2 );
	W[16] = gen_u( 2 );
	gen_vec( W + 17, -200.f, 200.f, 0 );
	W[20] = gen_bits( 0.f, 1200.f );
	W[21] = gen_bits( 0.f, 2.5f );
	W[22] = gen_bits( 0.f, 60.f );
	W[23] = gen_bits( -10.f, 120.f );
	W[24] = gen_bits( -1.f, 1.f );
	W[25] = gen_bits( 1.f, 80.f );
	W[26] = gen_bits( 1.f, 120.f );
	W[27] = gen_bits( 0.1f, 200.f );
	W[28] = gen_bits( 0.f, 300.f );
	W[29] = gen_u( 8 );
	W[30] = gen_u( 2 );
	W[31] = ClsLo + gen_u( ClsN );
}

static void gen_zone( uint32_t* W )
{
	gen_vec( W, -1500.f, 1500.f, 5 );
	gen_vec( W + 3, -400.f, 400.f, 20 );
	W[6] = gen_u( 2 );
}

static void gen_pool( uint32_t* W )
{
	for( int K = 0; K < 3; ++K )
	{
		W[K * 3 + 0] = gen_u( 5 );
		W[K * 3 + 1] = gen_bits( 0.1f, 300.f );
		W[K * 3 + 2] = gen_u( 2 );
	}
}

static void gen_hit( uint32_t* W )
{
	W[0] = (gen_u( 10 ) == 0) ? fbits( 1.0f ) : gen_bits( 0.f, 1.f );
	gen_vec( W + 1, -1.f, 1.f, 0 );
	gen_vec( W + 4, -200.f, 200.f, 0 );
	W[7] = gen_u( 3 );
}

static void gen_args( int F, uint32_t* A )
{
	switch( F )
	{
	case FN_CALC:
		gen_vec( A, -1.f, 1.f, 10 );
		A[3] = gen_bits( 0.f, 0.2f );
		A[4] = gen_bits( 0.f, 800.f );
		A[5] = gen_bits( 0.f, 4.f );
		A[6] = gen_u( 2 );
		A[7] = gen_u( 2 );
		A[8] = gen_u( 2 );
		break;
	case FN_MOUNT:
		gen_vec( A, -1.f, 1.f, 5 );
		break;
	case FN_STEP:
		gen_vec( A, -1.f, 1.f, 5 );
		gen_vec( A + 3, -1.f, 1.f, 5 );
		gen_vec( A + 6, -50.f, 50.f, 10 );
		break;
	case FN_HITWALL:
		gen_vec( A, -1.f, 1.f, 5 );
		A[3] = gen_u( 3 );
		break;
	case FN_LANDED:
		gen_vec( A, -1.f, 1.f, 5 );
		A[3] = (gen_u( 4 ) == 3) ? 0xffffffffu : gen_u( 3 );
		A[4] = gen_bits( 0.f, 0.1f );
		A[5] = gen_u( 6 );
		break;
	}
}

static void gen_case( int F, uint64_t Seed, uint32_t* W )
{
	g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
	bool Pawn = F <= FN_STEP;
	gen_subject( W, Pawn ? 1u : 0u, Pawn ? 2u : 5u );
	gen_zone( W + 32 );
	gen_pool( W + POOL_OFF );
	gen_hit( W + HIT_OFF );
	gen_args( F, W + ARG0 );
}

// ---- case execution -------------------------------------------------------------

static void run_case( int F, uint64_t Seed, const uint32_t* W, uint32_t* Out )
{
	memset( g_subj_mem, 0, sizeof g_subj_mem );
	memset( g_zone_mem, 0, sizeof g_zone_mem );
	memset( g_pool_mem, 0, sizeof g_pool_mem );
	AActor* S = (AActor*)g_subj_mem;
	g_zone = (AZoneInfo*)g_zone_mem;
	for( int K = 0; K < 3; ++K )
		g_pool[K] = (AActor*)g_pool_mem[K];

	load_zone( g_zone, W + 32 );
	for( int K = 0; K < 3; ++K )
		load_pool( g_pool[K], W + POOL_OFF + K * 3 );
	load_subject( S, W );
	for( int K = 0; K < 3; ++K ) *(void**)g_pool[K] = g_obj_vt;
	*(void**)S = g_obj_vt;
	*(void**)g_zone = g_obj_vt;

	FCheckResult Hit( fval( W[HIT_OFF] ) );
	Hit.Normal = vec3( W + HIT_OFF + 1 );
	Hit.Location = vec3( W + HIT_OFF + 4 );
	Hit.Actor = pool_or_null( W[HIT_OFF + 7] );

	g_svc = Seed;
	g_loglen = 0;
	g_log[0] = 0;
	uint32_t Ret = 0;
	const uint32_t* A = W + ARG0;
	switch( F )
	{
	case FN_CALC:
		((APawn*)S)->calcVelocity( vec3( A ), fval( A[3] ), fval( A[4] ), fval( A[5] ), (INT)A[6], (INT)A[7], (INT)A[8] );
		break;
	case FN_MOUNT:
		Ret = ((APawn*)S)->Mount( vec3( A ), Hit ) ? 1u : 0u;
		break;
	case FN_STEP:
		((APawn*)S)->stepUp( vec3( A ), vec3( A + 3 ), vec3( A + 6 ), Hit );
		break;
	case FN_HITWALL:
		S->processHitWall( vec3( A ), g_pool[A[3]] );
		break;
	case FN_LANDED:
		S->processLanded( vec3( A ), pool_or_null( A[3] ), fval( A[4] ), (INT)A[5] );
		break;
	}

	store_subject( S, Out );
	store_zone( g_zone, Out + 32 );
	store_hit( Hit, Out + 39 );
	Out[47] = Ret;
	uint64_t H = fnv1a( g_log, g_loglen );
	Out[48] = (uint32_t)(H >> 32);
	Out[49] = (uint32_t)H;
	Out[50] = (uint32_t)g_loglen;
}

static void print_case( int F, uint64_t Seed, const uint32_t* In, const uint32_t* Out )
{
	printf( "%s %016llx", kNames[F], (unsigned long long)Seed );
	for( int K = 0; K < ARG0 + kArgW[F]; ++K )
		printf( " %08x", In[K] );
	printf( " |" );
	for( int K = 0; K < OUT_W; ++K )
		printf( " %08x", Out[K] );
	printf( "\n" );
}

// The original PointRegion body is outside the linked UnPhysic.o. Keep the
// swim fixture's zone selection deterministic while exercising findWaterLine.
static char g_swim_model_mem[8192] __attribute__((aligned(16)));
static AZoneInfo* g_swim_other_zone;
FPointRegion UModel::PointRegion( AZoneInfo* Zone, FVector Location ) const
{
	(void)Zone;
	int Water = Location.Z < 0.f;
	lg( "point_region %08x %d\n", fbits( Location.X ), Water );
	AZoneInfo* ZoneAtPoint = g_zone->bWaterZone ? (Water ? g_zone : g_swim_other_zone) : (Water ? g_swim_other_zone : g_zone);
	return FPointRegion( ZoneAtPoint, 0, 0 );
}


// Route the original virtual performPhysics calls through their original bodies.
// The fixture objects share one synthetic vtable, so select the override by class.
static void hook_dispatch_perform_physics( AActor* Actor, FLOAT DeltaSeconds )
{
	uint32_t Class = class_code( Actor );
	if( Class == 1 || Class == 2 )
		((APawn*)Actor)->APawn::performPhysics( DeltaSeconds );
	else
		Actor->AActor::performPhysics( DeltaSeconds );
}

struct FDispatchVTableInstaller
{
	FDispatchVTableInstaller()
	{
		auto PmPerform = &AActor::performPhysics;
		g_obj_vt[vt_slot( &PmPerform )] = (void*)&hook_dispatch_perform_physics;
	}
};
static FDispatchVTableInstaller GDispatchVTableInstaller;

// The original engine's global math object is not linked into this harness.
// Keep its full layout and the trig entries used by interpolation initialized.
static void harness_setup_math_table( FLOAT* ManTbl, FLOAT* ExpTbl, FLOAT Power )
{
	union { FLOAT F; DWORD D; } Temp;
	Temp.F = 1.0f;
	for( DWORD i = 0; i < (1u << APPROX_EXP_BITS); ++i )
	{
		Temp.D = (Temp.D & 0x007fffff) + (i << (32 - APPROX_EXP_BITS));
		ExpTbl[i] = (FLOAT)std::pow(std::fabs(Temp.F), Power);
		if( std::isnan(ExpTbl[i]) ) ExpTbl[i] = 0.0f;
	}
	Temp.F = 1.0f;
	for( DWORD i = 0; i < (1u << APPROX_MAN_BITS); ++i )
	{
		Temp.D = (Temp.D & 0xff800000) + (i << (32 - APPROX_EXP_BITS - APPROX_MAN_BITS));
		ManTbl[i] = (FLOAT)std::pow(std::fabs(Temp.F), Power);
		if( std::isnan(ManTbl[i]) ) ManTbl[i] = 0.0f;
	}
}

FGlobalMath::FGlobalMath()
:	WorldMin(-32700.0,-32700.0,-32700.0),
	WorldMax(32700.0,32700.0,32700.0),
	UnitCoords(FVector(0,0,0),FVector(1,0,0),FVector(0,1,0),FVector(0,0,1)),
	UnitScale(FVector(1,1,1),0.0,SHEER_ZX),
	ViewCoords(FVector(0,0,0),FVector(0,1,0),FVector(0,0,-1),FVector(1,0,0))
{
	for( INT i = 0; i < NUM_ANGLES; ++i )
		TrigFLOAT[i] = sinf((FLOAT)i * 2.0f * PI / (FLOAT)NUM_ANGLES);
	union { FLOAT F; DWORD D; } Temp;
	for( DWORD D = 0; D < (1u << APPROX_MAN_BITS); ++D )
	{
		Temp.F = 1.0f;
		Temp.D = (Temp.D & 0xff800000) + (D << (23 - APPROX_MAN_BITS));
		Temp.F = (FLOAT)appSqrt(Temp.F);
		Temp.D -= (64u << 23);
		SqrtManTbl[D] = (FLOAT)(Temp.F * appSqrt(2.0));
		SqrtManTbl[D + (1u << APPROX_MAN_BITS)] = Temp.F * 2.0f;
	}
	harness_setup_math_table(DivSqrtManTbl, DivSqrtExpTbl, -0.5f);
	harness_setup_math_table(DivManTbl, DivExpTbl, -1.0f);
}
FGlobalMath GMath;

// Family files: each defines fam_<name>() and prints its fixture lines to stdout.
#include "physic_fam_motion.inc"
#include "physic_fam_walk.inc"
#include "physic_fam_fall.inc"
#include "physic_fam_swim.inc"
#include "physic_fam_move.inc"
#include "physic_fam_dispatch.inc"
#include "physic_fam_interp.inc"
#include "physic_fam_smooth.inc"

static int run_family( const char* Fam )
{
	if( !strcmp( Fam, "motion" ) ) { fam_motion(); return 0; }
	if( !strcmp( Fam, "walk" ) ) { fam_walk(); return 0; }
	if( !strcmp( Fam, "fall" ) ) { fam_fall(); return 0; }
	if( !strcmp( Fam, "swim" ) ) { fam_swim(); return 0; }
	if( !strcmp( Fam, "move" ) ) { fam_move(); return 0; }
	if( !strcmp( Fam, "dispatch" ) ) { fam_dispatch(); return 0; }
	if( !strcmp( Fam, "interp" ) ) { fam_interp(); return 0; }
	if( !strcmp( Fam, "smooth" ) ) { fam_smooth(); return 0; }
	return 2;
}

int main( int argc, char** argv )
{
	// Name indices: any distinct values; FindFunctionChecked compares against them.
	int32_t Idx;
	Idx = 1001; memcpy( &ENGINE_Mount, &Idx, sizeof Idx );
	Idx = 1002; memcpy( &ENGINE_HitWall, &Idx, sizeof Idx );
	Idx = 1003; memcpy( &ENGINE_Landed, &Idx, sizeof Idx );
	Idx = 1004; memcpy( &ENGINE_FellOutOfWorld, &Idx, sizeof Idx );
	Idx = 1005; memcpy( &ENGINE_Falling, &Idx, sizeof Idx );
	Idx = 1006; memcpy( &ENGINE_MayFall, &Idx, sizeof Idx );
	Idx = 1007; memcpy( &ENGINE_DoJump, &Idx, sizeof Idx );
	Idx = 1008; memcpy( &ENGINE_UpdateCamera, &Idx, sizeof Idx );
	Idx = 1009; memcpy( &ENGINE_InterpolateEnd, &Idx, sizeof Idx );
	Idx = 1010; memcpy( &ENGINE_FinishedInterpolation, &Idx, sizeof Idx );

	// Class chain: AActor <- APawn <- APlayerPawn; AActor <- ADecoration <- ACarcass.
	for( int K = 0; K < 5; ++K )
		g_cls[K] = (UClass*)g_cls_raw[K];
	((UClass*)g_cls_raw[1])->SuperField = (UClass*)g_cls_raw[0];
	((UClass*)g_cls_raw[2])->SuperField = (UClass*)g_cls_raw[1];
	((UClass*)g_cls_raw[3])->SuperField = (UClass*)g_cls_raw[0];
	((UClass*)g_cls_raw[4])->SuperField = (UClass*)g_cls_raw[3];

	install_level();
	const FCoords UnitCoords( FVector(0,0,0), FVector(1,0,0), FVector(0,1,0), FVector(0,0,1) );
	memcpy( (void*)&GMath.UnitCoords, &UnitCoords, sizeof UnitCoords );

	g_surf[0] = (FBspSurf*)calloc( 1, sizeof(FBspSurf) );
	g_surf[1] = (FBspSurf*)calloc( 1, sizeof(FBspSurf) );
	g_surf[1]->PolyFlags = PF_SpecialPoly;

	// Family selection: "scripted" (default) runs the table below; others run their own .inc.
	const char* Fam = argc > 1 ? argv[1] : "scripted";
	if( strcmp( Fam, "scripted" ) != 0 )
	{
		int Rc = run_family( Fam );
		if( Rc != 0 ) fprintf( stderr, "unknown family %s\n", Fam );
		return Rc;
	}

	uint64_t Master[FN_COUNT];
	for( int F = 0; F < FN_COUNT; ++F )
		Master[F] = 0x1F2E3D4C5B6A7988ULL ^ ((uint64_t)(F + 1) * 0x100000001B3ULL);

	uint32_t In[IN_MAX];
	uint32_t Out[OUT_W];
	for( int F = 0; F < FN_COUNT; ++F )
		for( int C = 0; C < kCasesPerFn; ++C )
		{
			uint64_t Seed = sm_next( &Master[F] );
			memset( In, 0, sizeof In );
			gen_case( F, Seed, In );
			run_case( F, Seed, In, Out );
			print_case( F, Seed, In, Out );
		}
	fprintf( stderr, "cases %d\n", FN_COUNT * kCasesPerFn );
	return 0;
}
