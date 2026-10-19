// Original-object FSceneNode fixture harness (UnCamera.cpp at 69c3263).
// gen_camera.sh rewrites the virtual boundary calls (RenDev->SetSceneNode, Draw2DLine,
// Draw2DPoint) into hooks and qualifies Draw2DClippedLine, then links the original objects;
// every arithmetic statement of ComputeRenderSize / ComputeRenderCoords / Draw3DLine runs unmodified.
// Families: render_size (ortho), render_size_persp (non-ortho, GetEffectiveFovAngle),
// render_coords (ComputeRenderCoords with GMath), draw3d (Draw3DLine emitted 2D calls).
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

// The original GMath lives in Core/Src/Core.cpp (not linked); UnMath.o supplies the constructor
// that fills TrigFLOAT, so the harness owns the real object.
FGlobalMath GMath;

static INT g_setscene_count = 0;
static uint32_t bits_of( FLOAT f ) { uint32_t u; memcpy( &u, &f, 4 ); return u; }
// Replaces `Viewport->RenDev->SetSceneNode( this )` in the rewritten original.
void CameraHookSetSceneNode( UViewport* Viewport, FSceneNode* Frame ) { g_setscene_count++; }

// Replaces `Draw2DLine( Frame, Color, LineFlags, P1, P2 )` in the rewritten original (virtual call).
struct Call2D { INT Kind; uint32_t W[12]; };  // Kind 0 = line (11 words), 1 = point (9 words)
static Call2D g_calls[16];
static INT g_ncalls = 0;
static void record_call( INT Kind, const uint32_t* W, INT N )
{
	if( g_ncalls >= 16 ) return;
	g_calls[g_ncalls].Kind = Kind;
	memcpy( g_calls[g_ncalls].W, W, N * sizeof(uint32_t) );
	g_ncalls++;
}
void CameraHookDraw2DLine( URenderDevice* RenDev, FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector P1, FVector P2 )
{
	const uint32_t W[11] = { bits_of( Color.X ), bits_of( Color.Y ), bits_of( Color.Z ), bits_of( Color.W ), LineFlags,
		bits_of( P1.X ), bits_of( P1.Y ), bits_of( P1.Z ), bits_of( P2.X ), bits_of( P2.Y ), bits_of( P2.Z ) };
	record_call( 0, W, 11 );
}
void CameraHookDraw2DPoint( URenderDevice* RenDev, FSceneNode* Frame, FPlane Color, DWORD LineFlags, FLOAT X1, FLOAT Y1, FLOAT X2, FLOAT Y2, FLOAT Z )
{
	const uint32_t W[10] = { bits_of( Color.X ), bits_of( Color.Y ), bits_of( Color.Z ), bits_of( Color.W ), LineFlags,
		bits_of( X1 ), bits_of( Y1 ), bits_of( X2 ), bits_of( Y2 ), bits_of( Z ) };
	record_call( 1, W, 10 );
}

// Verbatim from Engine/Src/UnCamMgr.cpp at 42a1e81^ (lines 37-45): the pre-port body that the
// Rust forwarder replaced. Not stubbed; appTan/appAtan resolve to UnAnsi.o.
FLOAT UClient::GetEffectiveFovAngle( FLOAT AuthoredHFov, INT ViewportX, INT ViewportY, UBOOL bMaintainVerticalFOV )
{
	if( !bMaintainVerticalFOV || ViewportX<=0 || ViewportY<=0 || 3.0*ViewportX<=4.0*ViewportY )
		return AuthoredHFov;

	const DOUBLE AspectScale = (3.0*ViewportX) / (4.0*ViewportY);
	const FLOAT EffectiveHFov = (FLOAT)(2.0 * appAtan(appTan(AuthoredHFov * PI/360.0) * AspectScale) * 180.0/PI);
	return Min( EffectiveHFov, 169.999f );
}

static uint32_t g_lcg = 0x2545F491u;
static uint32_t next_u32() { g_lcg = g_lcg * 1664525u + 1013904223u; return g_lcg; }
// Fractional value in [lo, hi); 24-bit mantissa-sized steps.
static FLOAT frand( FLOAT lo, FLOAT hi ) { return lo + (hi - lo) * (FLOAT)((next_u32() >> 8) / 16777216.0); }

static void set_coords( FCoords& C, const FLOAT* v )
{
	C.Origin = FVector( v[0], v[1], v[2] );
	C.XAxis = FVector( v[3], v[4], v[5] );
	C.YAxis = FVector( v[6], v[7], v[8] );
	C.ZAxis = FVector( v[9], v[10], v[11] );
}

static void print_coords( const char* label, const FCoords& C )
{
	const FLOAT V[12] = { C.Origin.X, C.Origin.Y, C.Origin.Z, C.XAxis.X, C.XAxis.Y, C.XAxis.Z,
		C.YAxis.X, C.YAxis.Y, C.YAxis.Z, C.ZAxis.X, C.ZAxis.Y, C.ZAxis.Z };
	printf( "%s", label );
	for( INT k = 0; k < 12; k++ ) printf( k ? ",%08x" : "%08x", bits_of( V[k] ) );
	printf( "\n" );
}

// Prints the 46 ComputeRenderSize output words, then the SetSceneNode count.
static void print_size_out( FSceneNode* Node )
{
	printf( "out" );
	const FLOAT Out[] = {
		Node->FX, Node->FY, Node->FX2, Node->FY2, Node->FX15, Node->FY15, Node->EffectiveFovAngle,
		Node->Proj.X, Node->Proj.Y, Node->Proj.Z,
		Node->RProj.X, Node->RProj.Y, Node->RProj.Z,
		Node->Zoom, Node->PrjXM, Node->PrjXP, Node->PrjYM, Node->PrjYP,
	};
	for( FLOAT v : Out ) printf( " %08x", bits_of( v ) );
	for( INT i = 0; i < 4; i++ )
		for( INT k = 0; k < 3; k++ )
		{
			FLOAT v = k == 0 ? Node->ViewSides[i].X : k == 1 ? Node->ViewSides[i].Y : Node->ViewSides[i].Z;
			printf( " %08x", bits_of( v ) );
		}
	for( INT i = 0; i < 4; i++ )
	{
		const FLOAT Pl[4] = { Node->ViewPlanes[i].X, Node->ViewPlanes[i].Y, Node->ViewPlanes[i].Z, Node->ViewPlanes[i].W };
		for( FLOAT v : Pl ) printf( " %08x", bits_of( v ) );
	}
	printf( "\n" );
	printf( "setscene %d\n", g_setscene_count );
}

// Builds a scene node with a calloc'd actor, viewport and (optional) client that owns MaintainVerticalFOV.
struct Rig
{
	APlayerPawn* Actor;
	UViewport* Viewport;
	UClient* Client;
	FSceneNode* Node;
};
static Rig make_rig( INT Map, FLOAT Fov, FLOAT Zoom, INT X, INT Y, INT Maintain )
{
	Rig R;
	R.Actor = (APlayerPawn*)calloc( 1, sizeof(APlayerPawn) );
	R.Actor->FovAngle = Fov;
	R.Actor->OrthoZoom = Zoom;
	R.Actor->RendMap = (BYTE)Map;
	R.Client = (UClient*)calloc( 1, sizeof(UClient) );
	R.Client->MaintainVerticalFOV = Maintain;
	R.Viewport = (UViewport*)calloc( 1, sizeof(UViewport) );
	R.Viewport->Actor = R.Actor;
	R.Viewport->Outer = R.Client;
	R.Node = (FSceneNode*)calloc( 1, sizeof(FSceneNode) );
	R.Node->Viewport = R.Viewport;
	R.Node->X = X;
	R.Node->Y = Y;
	return R;
}
static void free_rig( Rig& R )
{
	free( R.Node );
	free( R.Viewport );
	free( R.Client );
	free( R.Actor );
}

static const INT kSizes[8][2] = { {640,480}, {800,600}, {1024,768}, {1280,720}, {333,777}, {1,1}, {7,5}, {1600,900} };

int main()
{
	// ---- render_size (ortho): 48 cases, unchanged from the previous fixture ----
	{
		const INT kCases = 48;
		const INT Maps[3] = { REN_OrthXY, REN_OrthXZ, REN_OrthYZ };
		for( INT idx = 0; idx < kCases; idx++ )
		{
			const INT X = kSizes[idx % 8][0];
			const INT Y = kSizes[idx % 8][1];
			const INT Map = Maps[idx % 3];
			const FLOAT Fov = frand( 20.f, 150.f );
			const FLOAT Zoom = frand( 0.25f, 8.0f );
			FLOAT Coords[12], Uc[12];
			for( INT k = 0; k < 3; k++ ) Coords[k] = frand( -500.f, 500.f );
			for( INT k = 3; k < 12; k++ ) Coords[k] = frand( -1.f, 1.f );
			for( INT k = 0; k < 12; k++ ) Uc[k] = frand( -1.f, 1.f );

			Rig R = make_rig( Map, Fov, Zoom, X, Y, 0 );
			set_coords( R.Node->Coords, Coords );
			set_coords( R.Node->Uncoords, Uc );
			g_setscene_count = 0;
			R.Node->ComputeRenderSize();

			printf( "case render_size %d x=%d y=%d map=%d fov=%08x zoom=%08x coords=", idx, X, Y, Map, bits_of( Fov ), bits_of( Zoom ) );
			for( INT k = 0; k < 12; k++ ) printf( k ? ",%08x" : "%08x", bits_of( Coords[k] ) );
			printf( " uc=" );
			for( INT k = 0; k < 12; k++ ) printf( k ? ",%08x" : "%08x", bits_of( Uc[k] ) );
			printf( "\n" );
			print_size_out( R.Node );
			free_rig( R );
		}
	}

	// ---- render_size_persp (non-ortho, GetEffectiveFovAngle): 64 cases ----
	{
		const INT kCases = 64;
		const INT Maps[8] = { 0, 1, 3, 5, 6, 16, 18, 255 };
		for( INT idx = 0; idx < kCases; idx++ )
		{
			const INT X = kSizes[idx % 8][0];
			const INT Y = kSizes[idx % 8][1];
			const INT Map = Maps[(idx / 8) % 8];
			const INT Maintain = idx & 1;
			const FLOAT Fov = frand( 20.f, 170.f );
			const FLOAT Zoom = frand( 0.25f, 8.0f );
			FLOAT Coords[12], Uc[12];
			for( INT k = 0; k < 3; k++ ) Coords[k] = frand( -500.f, 500.f );
			for( INT k = 3; k < 12; k++ ) Coords[k] = frand( -1.f, 1.f );
			for( INT k = 0; k < 12; k++ ) Uc[k] = frand( -1.f, 1.f );

			Rig R = make_rig( Map, Fov, Zoom, X, Y, Maintain );
			set_coords( R.Node->Coords, Coords );
			set_coords( R.Node->Uncoords, Uc );
			g_setscene_count = 0;
			R.Node->ComputeRenderSize();

			printf( "case render_size_persp %d x=%d y=%d map=%d maintain=%d fov=%08x zoom=%08x coords=", idx, X, Y, Map, Maintain, bits_of( Fov ), bits_of( Zoom ) );
			for( INT k = 0; k < 12; k++ ) printf( k ? ",%08x" : "%08x", bits_of( Coords[k] ) );
			printf( " uc=" );
			for( INT k = 0; k < 12; k++ ) printf( k ? ",%08x" : "%08x", bits_of( Uc[k] ) );
			printf( "\n" );
			print_size_out( R.Node );
			free_rig( R );
		}
	}

	// ---- render_coords (ComputeRenderCoords: ortho literal coords or GMath.ViewCoords/Rotation): 48 cases ----
	{
		const INT kCases = 48;
		const INT Maps[6] = { REN_OrthXY, REN_OrthXZ, REN_OrthYZ, 3, 5, 16 };
		for( INT idx = 0; idx < kCases; idx++ )
		{
			const INT X = kSizes[idx % 8][0];
			const INT Y = kSizes[idx % 8][1];
			const INT Map = Maps[idx % 6];
			const INT Maintain = (idx / 6) & 1;
			const FLOAT Fov = frand( 20.f, 170.f );
			const FLOAT Zoom = frand( 0.25f, 8.0f );
			FVector Loc( frand( -300.f, 300.f ), frand( -300.f, 300.f ), frand( -300.f, 300.f ) );
			FRotator Rot;
			Rot.Pitch = (INT)next_u32();
			Rot.Yaw = (INT)next_u32();
			Rot.Roll = (INT)next_u32();

			Rig R = make_rig( Map, Fov, Zoom, X, Y, Maintain );
			g_setscene_count = 0;
			R.Node->ComputeRenderCoords( Loc, Rot );

			printf( "case render_coords %d x=%d y=%d map=%d maintain=%d fov=%08x zoom=%08x loc=%08x,%08x,%08x rot=%d,%d,%d\n",
				idx, X, Y, Map, Maintain, bits_of( Fov ), bits_of( Zoom ),
				bits_of( Loc.X ), bits_of( Loc.Y ), bits_of( Loc.Z ), Rot.Pitch, Rot.Yaw, Rot.Roll );
			print_coords( "coords ", R.Node->Coords );
			print_coords( "uc ", R.Node->Uncoords );
			print_size_out( R.Node );
			free_rig( R );
		}
	}

	// ---- draw3d (Draw3DLine -> emitted 2D calls): 96 cases, six branches ----
	{
		const INT kCases = 96;
		const char* Branch[6] = { "ortho_line", "ortho_dot", "ortho_dot_hidden", "persp_line", "persp_near_p1", "persp_discard" };
		const INT OrthoMaps[3] = { REN_OrthXY, REN_OrthXZ, REN_OrthYZ };
		for( INT idx = 0; idx < kCases; idx++ )
		{
			const INT B = idx % 6;
			const INT X = kSizes[idx % 8][0];
			const INT Y = kSizes[idx % 8][1];
			const bool Ortho = B < 3;
			const INT Map = Ortho ? OrthoMaps[idx % 3] : 3;
			const FLOAT Zoom = B == 2 ? 50000.f : frand( 0.25f, 8.0f );
			FVector P1( frand( -200.f, 200.f ), frand( -200.f, 200.f ), frand( -200.f, 200.f ) );
			FVector P2( frand( -200.f, 200.f ), frand( -200.f, 200.f ), frand( -200.f, 200.f ) );
			if( B == 1 || B == 2 ) P2 = FVector( P1.X + 1e-4f, P1.Y, P1.Z );
			if( B == 3 ) { P1.Z = frand( 1.f, 300.f ); P2.Z = frand( 1.f, 300.f ); }
			if( B == 4 ) { P1.Z = -5.f; P2.Z = frand( 10.f, 300.f ); }
			if( B == 5 ) { P1.Z = -5.f; P2.Z = -10.f; }
			const FLOAT Color[4] = { frand( 0.f, 1.f ), frand( 0.f, 1.f ), frand( 0.f, 1.f ), 1.f };
			const FLOAT Identity[12] = { 0,0,0, 1,0,0, 0,1,0, 0,0,1 };

			const FLOAT Fov = frand( 20.f, 170.f );
			Rig R = make_rig( Map, Fov, Zoom, X, Y, 0 );
			set_coords( R.Node->Coords, Identity );
			set_coords( R.Node->Uncoords, Identity );
			R.Node->ComputeRenderSize();

			g_ncalls = 0;
			{
				URenderDevice* RD = (URenderDevice*)calloc( 1, sizeof(URenderDevice) );
				RD->URenderDevice::Draw3DLine( R.Node, FPlane( Color[0], Color[1], Color[2], Color[3] ), LINE_None, P1, P2 );
				free( RD );
			}

			printf( "case draw3d %d branch=%s map=%d x=%d y=%d fov=%08x zoom=%08x color=%08x,%08x,%08x,%08x p1=%08x,%08x,%08x p2=%08x,%08x,%08x\n",
				idx, Branch[B], Map, X, Y, bits_of( Fov ), bits_of( Zoom ),
				bits_of( Color[0] ), bits_of( Color[1] ), bits_of( Color[2] ), bits_of( Color[3] ),
				bits_of( P1.X ), bits_of( P1.Y ), bits_of( P1.Z ), bits_of( P2.X ), bits_of( P2.Y ), bits_of( P2.Z ) );
			printf( "calls %d\n", g_ncalls );
			for( INT c = 0; c < g_ncalls; c++ )
			{
				const INT N = g_calls[c].Kind == 0 ? 11 : 10;
				printf( g_calls[c].Kind == 0 ? "line" : "point" );
				for( INT k = 0; k < N; k++ ) printf( " %08x", g_calls[c].W[k] );
				printf( "\n" );
			}
			free_rig( R );
		}
	}
	return 0;
}
