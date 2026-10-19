/*=============================================================================
	Hp2ParticleFxServices.h: C++ services for the Rust AParticleFX port.

	Consumed by crates/hp2-engine/src/particle_fx.rs (`Hp2ParticleFxServices`,
	#[repr(C)]). Field order must match the Rust side exactly. Every pointer is
	void* so the Rust exports take void*; vectors and coords are passed by
	address.
=============================================================================*/

#ifndef _INC_HP2PARTICLEFXSERVICES_H
#define _INC_HP2PARTICLEFXSERVICES_H

#include "Hp2ParticleServices.h"

struct Hp2ParticleFxServices
{
	const Hp2ParticleServices* Particle;

	float  (*AppFrandRange)( float From, float To );
	int    (*AppRandRangeInt)( int Min, int Max );
	double (*AppExp)( double Value );
	float  (*SqrtApprox)( float F );
	float  (*SinFloat)( float F );
	float  (*CosFloat)( float F );

	void   (*ActorToLocal)( void* Actor, void* OutCoords );
	void   (*ActorZoneGravity)( void* Actor, void* OutVector );
	float  (*ActorLodBias)( void* Actor );
	double (*LevelTimeSeconds)( void* Actor );
	void   (*LevelDestroyActor)( void* Actor );
	int    (*IsEditor)();
	void   (*SyncOwnerTransient)( void* Actor );
	void*  (*ParentDefaults)( void* Actor );

	void   (*MeshRenderBox)( void* Owner, void* OutCoords );
	int    (*MeshFrameVerts)( void* Owner );
	void   (*MeshGetFrame)( void* Owner, void* OutVerts );
	int    (*MeshNumTris)( void* Owner );
	void   (*MeshTriVerts)( void* Owner, int Tri, int* OutVerts );

	int    (*PatternNumPoints)( void* Pattern );
	void   (*PatternPoint)( void* Pattern, int I, void* OutVector );

	int    (*PaletteColor0)( void* ColorPalette, void* OutPlane );

	void*  (*ListStart)( void* List );
	void*  (*ListNext)( void* List );
	void   (*ListAdd)( void* List, const void* Particle );
	void   (*ListRemoveCurrent)( void* List );
	void   (*ListRemoveAll)( void* List );
	void   (*ListRemoveFromFront)( void* List );
	int    (*ListSize)( void* List );
	void*  (*ListGetParticle)( void* List, int Id );
	void   (*ListUpdateBox)( void* List );
	void   (*ListWorldBox)( void* List, void* OutBox );

	int    (*EmitParticlesShell)( void* System, float Tick );
	int    (*UpdateShell)( void* System, float Step );

	void   (*StatClockUpdate)( void );
	void   (*StatUnclockUpdate)( void );
	void   (*StatParticlesEmitted)( int N );
};

extern const Hp2ParticleFxServices GHp2ParticleFxServices;

#endif
