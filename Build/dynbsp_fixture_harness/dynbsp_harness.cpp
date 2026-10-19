// Oracle harness for FMovingBrushTracker (UnDynBsp.cpp at HEAD).
//
// Builds a static box BSP world plus mover brushes from real engine types. UObject-derived
// storage is zeroed and never constructed; the tracker only needs IsA (non-virtual, walks
// Class -> SuperField), so no vtables are required. The original tracker is driven through a
// fixed op script and the full tracker-visible state is printed after every op. The generator
// (gen_dynbsp_fixtures.sh) writes this text to crates/hp2-engine/tests/fixtures/. The Rust test
// replays the same ops against the Rust port.
//
// Access specifiers do not change layout (Itanium ABI); the harness needs UObject::Class.
#define private public
#define protected public
#include "EnginePrivate.h"
#undef private
#undef protected

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

// ---------------------------------------------------------------------------
// Process globals. Core/UnMath/UnMem objects provide the rest.
// ---------------------------------------------------------------------------

class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { return malloc( Count ? Count : 1 ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag )
	{
		if( Count == 0 ) { free( Original ); return NULL; }
		return realloc( Original, Count );
	}
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc GHarnessMalloc;

// Class objects: zeroed storage. PrivateStaticClass is a UClass* pointer symbol (STATIC_LINK
// build); it is set non-null here so the inline StaticClass() never takes the registration path
// (GetPrivateStaticClass*, which is not linked). UObject::PrivateStaticClass comes from real
// Core/UnObj.o and is not used.
alignas(16) static unsigned char gAActorObj[sizeof( UClass )];
alignas(16) static unsigned char gABrushObj[sizeof( UClass )];
alignas(16) static unsigned char gAMoverObj[sizeof( UClass )];
UClass* gAActorPtr __asm__( "__ZN6AActor18PrivateStaticClassE" ) = reinterpret_cast<UClass*>( gAActorObj );
UClass* gABrushPtr __asm__( "__ZN6ABrush18PrivateStaticClassE" ) = reinterpret_cast<UClass*>( gABrushObj );
UClass* gAMoverPtr __asm__( "__ZN6AMover18PrivateStaticClassE" ) = reinterpret_cast<UClass*>( gAMoverObj );

// ---------------------------------------------------------------------------
// Dump helpers: floats as u32 bit patterns in hex; integers decimal.
// ---------------------------------------------------------------------------

static void hexf( FLOAT f )
{
	uint32_t u;
	memcpy( &u, &f, 4 );
	printf( "%08x", u );
}
static void hv( const FVector& v )
{
	printf( " " );
	hexf( v.X ); printf( " " ); hexf( v.Y ); printf( " " ); hexf( v.Z );
}
static void hp( const FPlane& p )
{
	printf( " " );
	hexf( p.X ); printf( " " ); hexf( p.Y ); printf( " " ); hexf( p.Z ); printf( " " ); hexf( p.W );
}
static void hc( const FCoords& c )
{
	hv( c.Origin ); hv( c.XAxis ); hv( c.YAxis ); hv( c.ZAxis );
}
static void hr( const FRotator& r )
{
	printf( " %d %d %d", r.Pitch, r.Yaw, r.Roll );
}

// ---------------------------------------------------------------------------
// World: a box whose six faces are static BSP nodes. Node k's front is outside (empty), its
// back is node k+1, and the last back is solid.
// ---------------------------------------------------------------------------

static const FLOAT BOX_HALF = 256.0f;
static const FLOAT MOVER_HALF = 48.0f;
static const int MAX_MOVERS = 4;

struct FaceDef { FVector N, U, V; };
// u x v = n for each face, so the vertex order below is CCW seen from outside.
static const FaceDef FACES[6] = {
	{ FVector( 1, 0, 0 ), FVector( 0, 1, 0 ), FVector( 0, 0, 1 ) },
	{ FVector( -1, 0, 0 ), FVector( 0, 0, 1 ), FVector( 0, 1, 0 ) },
	{ FVector( 0, 1, 0 ), FVector( 0, 0, 1 ), FVector( 1, 0, 0 ) },
	{ FVector( 0, -1, 0 ), FVector( 1, 0, 0 ), FVector( 0, 0, 1 ) },
	{ FVector( 0, 0, 1 ), FVector( 1, 0, 0 ), FVector( 0, 1, 0 ) },
	{ FVector( 0, 0, -1 ), FVector( 0, 1, 0 ), FVector( 1, 0, 0 ) },
};

alignas(16) static unsigned char gLevelBuf[sizeof( ULevel )];
alignas(16) static unsigned char gWorldModelBuf[sizeof( UModel )];
alignas(16) static unsigned char gMoverBufs[MAX_MOVERS][sizeof( AMover )];
alignas(16) static unsigned char gMoverModelBufs[MAX_MOVERS][sizeof( UModel )];
alignas(16) static unsigned char gMoverPolysBufs[MAX_MOVERS][sizeof( UPolys )];
alignas(16) static unsigned char gPlainBuf[sizeof( AActor )];

static ULevel* gLevel;
static UModel* gWorld;
static AActor* gActors[MAX_MOVERS + 1];
static int gNumActors;

// debugf logs Level->GetFullName(). The real UObject::GetFullName lives in UnObj.cpp, which is not
// linked (it drags in the whole object system). The log text never reaches the fixture.
const TCHAR* UObject::GetFullName( TCHAR* Str ) const
{
	return Str ? Str : TEXT( "harness" );
}
static AMover* gMovers[MAX_MOVERS];
static UModel* gMoverModels[MAX_MOVERS];
static int gNumMovers;

static void zeroFill( void* p, size_t n ) { memset( p, 0, n ); }

static void buildBrush( UModel* model, UPolys* polys )
{
	zeroFill( polys, sizeof( UPolys ) );
	model->Polys = polys;
	model->MoverLink = INDEX_NONE;
	polys->Element.AddZeroed( 6 );
	static const int S[4][2] = { { -1, -1 }, { 1, -1 }, { 1, 1 }, { -1, 1 } };
	for( int i = 0; i < 6; i++ )
	{
		const FaceDef& f = FACES[i];
		FPoly& p = polys->Element( i );
		memset( &p, 0, sizeof( FPoly ) );
		FVector center( f.N.X * MOVER_HALF, f.N.Y * MOVER_HALF, f.N.Z * MOVER_HALF );
		for( int j = 0; j < 4; j++ )
		{
			p.Vertex[j] = FVector(
				center.X + MOVER_HALF * ( S[j][0] * f.U.X + S[j][1] * f.V.X ),
				center.Y + MOVER_HALF * ( S[j][0] * f.U.Y + S[j][1] * f.V.Y ),
				center.Z + MOVER_HALF * ( S[j][0] * f.U.Z + S[j][1] * f.V.Z ) );
		}
		p.Base = p.Vertex[0];
		p.Normal = f.N;
		p.TextureU = f.U;
		p.TextureV = f.V;
		p.PolyFlags = 0;
		p.Actor = NULL;
		p.Texture = NULL;
		p.NumVertices = 4;
		p.iLink = INDEX_NONE;
		p.iBrushPoly = i;
		p.PanU = 0;
		p.PanV = 0;
		p.SavePolyIndex = i;
		p.bFaceDragSel = 0;
	}
}

static void buildWorld()
{
	zeroFill( gWorldModelBuf, sizeof( UModel ) );
	gWorld = (UModel*)gWorldModelBuf;
	gWorld->RootOutside = 1;
	gWorld->Linked = 0;
	gWorld->MoverLink = INDEX_NONE;
	gWorld->Polys = NULL;

	gWorld->Points.AddZeroed( 6 );
	gWorld->Vectors.AddZeroed( 18 );
	for( int i = 0; i < 6; i++ )
	{
		const FaceDef& f = FACES[i];
		gWorld->Points( i ) = FVector( f.N.X * BOX_HALF, f.N.Y * BOX_HALF, f.N.Z * BOX_HALF );
		gWorld->Vectors( 3 * i + 0 ) = f.N;
		gWorld->Vectors( 3 * i + 1 ) = f.U;
		gWorld->Vectors( 3 * i + 2 ) = f.V;
	}

	gWorld->Surfs.AddZeroed( 6 );
	for( int i = 0; i < 6; i++ )
	{
		FBspSurf& s = gWorld->Surfs( i );
		memset( &s, 0, sizeof( FBspSurf ) );
		s.PolyFlags = 0;
		s.pBase = i;
		s.vNormal = 3 * i + 0;
		s.vTextureU = 3 * i + 1;
		s.vTextureV = 3 * i + 2;
		s.iLightMap = INDEX_NONE;
		s.iBrushPoly = INDEX_NONE;
		s.Actor = NULL;
	}

	gWorld->Nodes.AddZeroed( 6 );
	for( int i = 0; i < 6; i++ )
	{
		const FaceDef& f = FACES[i];
		FBspNode& n = gWorld->Nodes( i );
		memset( &n, 0, sizeof( FBspNode ) );
		FVector base( f.N.X * BOX_HALF, f.N.Y * BOX_HALF, f.N.Z * BOX_HALF );
		n.Plane = FPlane( base, f.N );
		n.ZoneMask = 3;
		n.iVertPool = INDEX_NONE;
		n.iSurf = i;
		n.iBack = ( i < 5 ) ? i + 1 : INDEX_NONE;
		n.iFront = INDEX_NONE;
		n.iPlane = INDEX_NONE;
		n.iCollisionBound = INDEX_NONE;
		n.iRenderBound = INDEX_NONE;
		n.iZone[0] = 0;
		n.iZone[1] = 1;
		n.NumVertices = 4;
		n.NodeFlags = 0;
		n.iLeaf[0] = INDEX_NONE;
		n.iLeaf[1] = INDEX_NONE;
	}
}

// Level actors in Level->Actors order. Movers come first; with plainActor, a non-brush AActor
// is inserted after the first mover to exercise the IsMovingBrush false branch.
static void buildLevel( int numMovers, bool plainActor )
{
	zeroFill( gLevelBuf, sizeof( ULevel ) );
	gLevel = (ULevel*)gLevelBuf;
	gLevel->Model = gWorld;
	gNumActors = 0;
	gNumMovers = 0;
	for( int m = 0; m < numMovers; m++ )
	{
		AMover* mv = (AMover*)gMoverBufs[m];
		zeroFill( mv, sizeof( AMover ) );
		UModel* model = (UModel*)gMoverModelBufs[m];
		zeroFill( model, sizeof( UModel ) );
		buildBrush( model, (UPolys*)gMoverPolysBufs[m] );
		mv->Class = gAMoverPtr;
		mv->Brush = model;
		mv->bStatic = 0;
		mv->Location = FVector( 0, 0, 0 );
		mv->Rotation = FRotator( 0, 0, 0 );
		mv->PrePivot = FVector( 8, -4, 12 );
		// Same as ABrush::InitPosRotScale (MainScale = PostScale = GMath.UnitScale); zero scales give NaN coords.
		mv->MainScale = FScale( FVector( 1, 1, 1 ), 0.0, SHEER_ZX );
		mv->PostScale = FScale( FVector( 1, 1, 1 ), 0.0, SHEER_ZX );
		gMovers[gNumMovers] = mv;
		gMoverModels[gNumMovers] = model;
		gNumMovers++;
		gActors[gNumActors] = (AActor*)mv;
		gLevel->Actors.AddItem( (AActor*)mv );
		gNumActors++;
		if( plainActor && m == 0 )
		{
			AActor* pa = (AActor*)gPlainBuf;
			zeroFill( pa, sizeof( AActor ) );
			pa->Class = gAActorPtr;
			gActors[gNumActors] = pa;
			gLevel->Actors.AddItem( pa );
			gNumActors++;
		}
	}
}

// ---------------------------------------------------------------------------
// Dump
// ---------------------------------------------------------------------------

static int actorIndex( const AActor* a )
{
	if( !a ) return -1;
	for( int i = 0; i < gNumActors; i++ )
		if( gActors[i] == a ) return i;
	return -2;
}

static int moverIndex( const AActor* a )
{
	for( int i = 0; i < gNumMovers; i++ )
		if( gMovers[i] == (AMover*)a ) return i;
	return -1;
}

static int modelIndex( const UModel* m )
{
	if( !m ) return -1;
	if( m == gWorld ) return -3;
	for( int i = 0; i < gNumMovers; i++ )
		if( gMoverModels[i] == m ) return i;
	return -2;
}

static void dumpState( FMovingBrushTrackerBase* tracker )
{
	UModel* model = gWorld;
	for( int i = 0; i < model->Surfs.Num(); i++ )
	{
		FBspSurf& s = model->Surfs( i );
		printf( "SURF %d %d %u %d %d %d %d %d %d %d %d %d %d %d\n",
			i, s.Texture ? 1 : 0, s.PolyFlags, s.pBase, s.vNormal, s.vTextureU, s.vTextureV,
			s.iLightMap, s.iBrushPoly, (int)s.PanU, (int)s.PanV, actorIndex( s.Actor ),
			s.Decals.Num(), s.Nodes.Num() );
		if( tracker )
			printf( "SDYN %d %d\n", i, (int)tracker->SurfIsDynamic( i ) );
	}
	for( int i = 0; i < model->Vectors.Num(); i++ ) { printf( "VEC %d", i ); hv( model->Vectors( i ) ); printf( "\n" ); }
	for( int i = 0; i < model->Points.Num(); i++ ) { printf( "PT %d", i ); hv( model->Points( i ) ); printf( "\n" ); }
	for( int i = 0; i < model->Nodes.Num(); i++ )
	{
		FBspNode& n = model->Nodes( i );
		printf( "NODE %d", i ); hp( n.Plane );
		printf( " %llu %d %d %d %d %d %d %d %d %d %d %d %d %d\n",
			(unsigned long long)n.ZoneMask, n.iVertPool, n.iSurf, n.iBack, n.iFront, n.iPlane,
			n.iCollisionBound, n.iRenderBound, (int)n.iZone[0], (int)n.iZone[1],
			(int)n.NumVertices, (int)n.NodeFlags, n.iLeaf[0], n.iLeaf[1] );
	}
	for( int i = 0; i < model->Verts.Num(); i++ )
	{
		FVert& v = model->Verts( i );
		printf( "VERT %d %d %d\n", i, v.pVertex, v.iSide );
	}
	for( int i = 0; i < gNumActors; i++ )
	{
		AMover* mv = (AMover*)gActors[i];
		if( moverIndex( gActors[i] ) < 0 ) { printf( "ACTOR %d plain\n", i ); continue; }
		printf( "ACTOR %d mover", i ); hv( mv->Location ); hr( mv->Rotation ); hv( mv->PrePivot );
		hv( mv->SavedPos ); hr( mv->SavedRot );
		printf( " %d %d\n", (int)mv->bAssimilated, (int)mv->bSpecialLit );
		FModelCoords mc;
		FLOAT orient = ( (ABrush*)mv )->BuildCoords( &mc, NULL );
		printf( "COORDS %d", i ); hc( mc.PointXform ); printf( " " ); hc( mc.VectorXform ); printf( " " );
		hexf( orient ); printf( "\n" );
		printf( "BRUSH %d %d\n", i, modelIndex( mv->Brush ) );
	}
	for( int i = 0; i < gNumMovers; i++ )
	{
		UModel* m = gMoverModels[i];
		printf( "MODEL %d link %d polys %d\n", i, m->MoverLink, m->Polys->Element.Num() );
		for( int j = 0; j < m->Polys->Element.Num(); j++ )
		{
			FPoly& p = m->Polys->Element( j );
			printf( "BPOLY %d %d", i, j );
			hv( p.Base ); hv( p.Normal ); hv( p.TextureU ); hv( p.TextureV );
			for( int k = 0; k < FPoly::MAX_VERTICES; k++ ) hv( p.Vertex[k] );
			printf( " %u %d %d %d %d %d %d %d %d\n", p.PolyFlags, actorIndex( p.Actor ), p.NumVertices,
				p.iLink, p.iBrushPoly, (int)p.PanU, (int)p.PanV, p.SavePolyIndex, (int)p.bFaceDragSel );
		}
	}
	int live = 0;
	if( tracker )
	{
		// Sporadic nodes owned by movers: walk each mover brush's MoverLink chain (iRenderBound links).
		for( int i = 0; i < gNumMovers; i++ )
		{
			int n = gMoverModels[i]->MoverLink;
			for( int g = 0; n != -1 && g < model->Nodes.Num(); g++ )
			{
				live++;
				n = model->Nodes( n ).iRenderBound;
			}
		}
	}
	printf( "LIVE %d\n", live );
	printf( "END_STATE\n" );
}

// ---------------------------------------------------------------------------
// Script
// ---------------------------------------------------------------------------

static void pose( int moverIdx, const FVector& loc, int pitch, int yaw, int roll )
{
	AMover* mv = gMovers[moverIdx];
	mv->Location = loc;
	mv->Rotation = FRotator( pitch, yaw, roll );
	printf( "OP POSE %d", moverIdx );
	hv( loc );
	hr( mv->Rotation );
	printf( "\n" );
}

static void opDump( const char* tag, FMovingBrushTrackerBase* tracker )
{
	printf( "OP %s\n", tag );
	dumpState( tracker );
}

// Mover ordinal -> actor pointer (movers are the first actors in the level).
static AActor* moverActor( int moverIdx ) { return (AActor*)gMovers[moverIdx]; }

static void runCase( const char* name, int numMovers, bool plainActor, int script )
{
	printf( "CASE %s\n", name );
	buildLevel( numMovers, plainActor );
	opDump( "INIT", NULL );

	FMovingBrushTrackerBase* tracker = GNewBrushTracker( gLevel );
	opDump( "CTOR", tracker );

	switch( script )
	{
	case 0:  // one mover inside the box, slide and rotate, update all, flush (no insertion expected)
		pose( 0, FVector( 120, -30, 60 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		// Yaw 8192 = 45 degrees; x=150 keeps every corner well short of the +X wall at x=256.
		pose( 0, FVector( 150, 40, -10 ), 0, 8192, 0 );
		tracker->Update( NULL );
		opDump( "UPDATE -1", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	case 1:  // two movers, one crossing +X, one crossing -Z, plus a plain actor in between
		pose( 0, FVector( 230, 0, 0 ), 16384, 4096, 0 );
		pose( 1, FVector( -250, 60, -200 ), 0, 0, 8192 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Update( moverActor( 1 ) );
		opDump( "UPDATE 1", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		pose( 0, FVector( 0, 0, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		break;
	case 2:  // coplanar front: +X face at x = Loc.x - PrePivot.x + MOVER_HALF = 256 exactly, normal +X
		pose( 0, FVector( 216, 0, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	case 3:  // fully outside +X: every fragment lands on the front of node 0
		pose( 0, FVector( 330, 0, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	case 4:  // straddles the +X (x=256) and +Y (y=256) walls: polys split at node 0 and node 2
		pose( 0, FVector( 256, 252, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	case 5:  // coplanar back: -X face at x = Loc.x - PrePivot.x - MOVER_HALF = 256, normal -X (faces away)
		pose( 0, FVector( 312, 0, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	case 6:  // three movers outside three different walls (+X, -X, -Z)
		pose( 0, FVector( 330, 0, 0 ), 0, 0, 0 );
		pose( 1, FVector( -330, 0, 0 ), 0, 0, 0 );
		pose( 2, FVector( 0, 0, -330 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Update( moverActor( 1 ) );
		opDump( "UPDATE 1", tracker );
		tracker->Update( moverActor( 2 ) );
		opDump( "UPDATE 2", tracker );
		tracker->Flush( moverActor( 1 ) );
		opDump( "FLUSH 1", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	case 7:  // outside +X, flush to zero, then outside -Y: re-adds after a flush
		pose( 0, FVector( 330, 0, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		pose( 0, FVector( 0, -330, 0 ), 0, 0, 0 );
		tracker->Update( moverActor( 0 ) );
		opDump( "UPDATE 0", tracker );
		tracker->Flush( moverActor( 0 ) );
		opDump( "FLUSH 0", tracker );
		break;
	}

	delete tracker;
	opDump( "DTOR", NULL );
	printf( "END\n" );
}

// Engine global (Engine.cpp defines it; UnEngine.cpp Inits it with 64 KiB chunks).
FMemStack GEngineMem;

int main()
{
	GMalloc = &GHarnessMalloc;
	GEngineMem.Init( 65536, TEXT("GEngineMem") );
	// IsA(ABrush::StaticClass()) on a mover walks AMover -> ABrush (SuperField).
	gAMoverPtr->SuperField = gABrushPtr;
	gABrushPtr->SuperField = gAActorPtr;
	buildWorld();
	runCase( "single_mover_inside_rotate", 1, false, 0 );
	runCase( "two_movers_partial_outside", 2, true, 1 );
	runCase( "coplanar_front_inside", 1, false, 2 );
	runCase( "mover_outside_plus_x", 1, false, 3 );
	runCase( "mover_straddles_plus_xy", 1, false, 4 );
	runCase( "coplanar_back_outside", 1, false, 5 );
	runCase( "three_movers_outside", 3, false, 6 );
	runCase( "regrow_after_flush", 1, false, 7 );
	return 0;
}
