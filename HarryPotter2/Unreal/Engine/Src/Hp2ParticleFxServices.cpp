/*=============================================================================
	Hp2ParticleFxServices.cpp: Thunks the Rust AParticleFX port calls back into.
	Each thunk performs one of the original engine operations unchanged.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2ParticleServices.h"
#include "Hp2ParticleFxServices.h"
#include "UnParticle.h"
#include "UnParticleList.h"
#include "UnMesh.h"

#include "UnStat.h"
// Mirrors of the Rust #[repr(C)] layouts in crates/hp2-engine/src/particle_fx.rs.
static_assert( sizeof(FParams) == 76, "FParams layout drifted from particle_fx.rs" );
static_assert( sizeof(FParticleParams) == 52, "FParticleParams layout drifted from particle_fx.rs" );

static float ParticleFxAppFrandRange( float From, float To )
{
	return appFrand( From, To );
}

static int ParticleFxAppRandRangeInt( int Min, int Max )
{
	return appRandRange( Min, Max );
}

static double ParticleFxAppExp( double Value )
{
	return appExp( Value );
}

static float ParticleFxSqrtApprox( float F )
{
	return GMath.SqrtApprox( F );
}

static float ParticleFxSinFloat( float F )
{
	return GMath.SinFloat( F );
}

static float ParticleFxCosFloat( float F )
{
	return GMath.CosFloat( F );
}

static void ParticleFxActorToLocal( void* Actor, void* OutCoords )
{
	*(FCoords*)OutCoords = ((AActor*)Actor)->ToLocal();
}

// Verbatim from AParticleFX::EmitParticles / UpdateParticles: Region.Zone ? Region.Zone : Level.
static void ParticleFxActorZoneGravity( void* Actor, void* OutVector )
{
	AActor* A = (AActor*)Actor;
	AZoneInfo* Info = A->Region.Zone ? A->Region.Zone : A->Level;
	*(FVector*)OutVector = Info->ZoneGravity;
}

static float ParticleFxActorLodBias( void* Actor )
{
	return ((AActor*)Actor)->LODBias;
}

static double ParticleFxLevelTimeSeconds( void* Actor )
{
	return ((AActor*)Actor)->Level->TimeSeconds;
}

static void ParticleFxLevelDestroyActor( void* Actor )
{
	((AParticleFX*)Actor)->GetLevel()->DestroyActor( (AActor*)Actor );
}

static int ParticleFxIsEditor()
{
	return GIsEditor ? 1 : 0;
}

// Verbatim from AParticleFX::Update (UnParticleFX.cpp:713-714).
static void ParticleFxSyncOwnerTransient( void* Actor )
{
	AParticleFX* Sys = (AParticleFX*)Actor;
	if( Sys->Owner && Sys->Base && (Sys->Owner == Sys->Base) && (Sys->Owner->GetFlags() & RF_Transient) )
		Sys->SetFlags( RF_Transient );
}

static void* ParticleFxParentDefaults( void* Actor )
{
	return ((AParticleFX*)Actor)->GetClass()->GetSuperClass()->Defaults.GetData();
}

static void ParticleFxMeshRenderBox( void* Owner, void* OutCoords )
{
	AActor* A = (AActor*)Owner;
	*(FCoords*)OutCoords = A->Mesh->GetRenderBoundingBox( A, 0 );
}

static int ParticleFxMeshFrameVerts( void* Owner )
{
	return ((AActor*)Owner)->Mesh->FrameVerts;
}

static void ParticleFxMeshGetFrame( void* Owner, void* OutVerts )
{
	AActor* A = (AActor*)Owner;
	A->Mesh->GetFrame( (FVector*)OutVerts, sizeof(FVector), GMath.UnitCoords, A );
}

static int ParticleFxMeshNumTris( void* Owner )
{
	return ((AActor*)Owner)->Mesh->GetNumTris();
}

static void ParticleFxMeshTriVerts( void* Owner, int Tri, int* OutVerts )
{
	((AActor*)Owner)->Mesh->GetTriVerts( Tri, OutVerts );
}

static int ParticleFxPatternNumPoints( void* Pattern )
{
	return ((UGesture*)Pattern)->Points.Num();
}

static void ParticleFxPatternPoint( void* Pattern, int I, void* OutVector )
{
	*(FVector*)OutVector = ((UGesture*)Pattern)->Points( I );
}

// Verbatim from the palette branches of AParticleFX::EmitParticles / AddParticle: writes
// ColorPalette->Palette->Colors(0).Plane() and returns 1 when the palette is present.
static int ParticleFxPaletteColor0( void* ColorPalette, void* OutPlane )
{
	UTexture* Tex = (UTexture*)ColorPalette;
	if( Tex->Palette )
	{
		*(FPlane*)OutPlane = Tex->Palette->Colors(0).Plane();
		return 1;
	}
	return 0;
}

static void* ParticleFxListStart( void* List )
{
	return ((UParticleList*)List)->StartParticle();
}

static void* ParticleFxListNext( void* List )
{
	return ((UParticleList*)List)->NextParticle();
}

static void ParticleFxListAdd( void* List, const void* Particle )
{
	((UParticleList*)List)->Add( *(UParticle*)Particle );
}

static void ParticleFxListRemoveCurrent( void* List )
{
	((UParticleList*)List)->RemoveCurrent();
}

static void ParticleFxListRemoveAll( void* List )
{
	((UParticleList*)List)->RemoveAll();
}

static void ParticleFxListRemoveFromFront( void* List )
{
	((UParticleList*)List)->RemoveFromFront();
}

static int ParticleFxListSize( void* List )
{
	return ((UParticleList*)List)->Size();
}

static void* ParticleFxListGetParticle( void* List, int Id )
{
	return ((UParticleList*)List)->GetParticle( Id );
}

static void ParticleFxListUpdateBox( void* List )
{
	((UParticleList*)List)->UpdateBox();
}

static void ParticleFxListWorldBox( void* List, void* OutBox )
{
	*(FBox*)OutBox = ((UParticleList*)List)->WorldBox;
}

static int ParticleFxEmitParticlesShell( void* System, float Tick )
{
	return ((AParticleFX*)System)->EmitParticles( Tick );
}

static int ParticleFxUpdateShell( void* System, float Step )
{
	return ((AParticleFX*)System)->Update( Step ) ? 1 : 0;
}

static void ParticleFxStatClockUpdate( void )
{
	clock(GStat.ParticleUpdateTime);
}

static void ParticleFxStatUnclockUpdate( void )
{
	unclock(GStat.ParticleUpdateTime);
}

static void ParticleFxStatParticlesEmitted( int N )
{
	GStat.ParticlesEmitted += N;
}

const Hp2ParticleFxServices GHp2ParticleFxServices =
{
	&GHp2ParticleServices,
	ParticleFxAppFrandRange,
	ParticleFxAppRandRangeInt,
	ParticleFxAppExp,
	ParticleFxSqrtApprox,
	ParticleFxSinFloat,
	ParticleFxCosFloat,
	ParticleFxActorToLocal,
	ParticleFxActorZoneGravity,
	ParticleFxActorLodBias,
	ParticleFxLevelTimeSeconds,
	ParticleFxLevelDestroyActor,
	ParticleFxIsEditor,
	ParticleFxSyncOwnerTransient,
	ParticleFxParentDefaults,
	ParticleFxMeshRenderBox,
	ParticleFxMeshFrameVerts,
	ParticleFxMeshGetFrame,
	ParticleFxMeshNumTris,
	ParticleFxMeshTriVerts,
	ParticleFxPatternNumPoints,
	ParticleFxPatternPoint,
	ParticleFxPaletteColor0,
	ParticleFxListStart,
	ParticleFxListNext,
	ParticleFxListAdd,
	ParticleFxListRemoveCurrent,
	ParticleFxListRemoveAll,
	ParticleFxListRemoveFromFront,
	ParticleFxListSize,
	ParticleFxListGetParticle,
	ParticleFxListUpdateBox,
	ParticleFxListWorldBox,
	ParticleFxEmitParticlesShell,
	ParticleFxUpdateShell,

	ParticleFxStatClockUpdate,
	ParticleFxStatUnclockUpdate,
	ParticleFxStatParticlesEmitted,
};
