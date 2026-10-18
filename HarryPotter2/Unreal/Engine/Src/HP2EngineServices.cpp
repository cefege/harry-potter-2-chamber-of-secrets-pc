/*=============================================================================
	HP2EngineServices.cpp: Engine-global services called by the Rust engine port
	(crates/hp2-engine). Each adapter decodes raw pointers into the C++ types and
	forwards to the engine-owned implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "HP2EngineServices.h"

static FLOAT EngineAppFrand()
{
	return appFrand();
}

static INT EngineAppRand()
{
	return appRand();
}

static INT EngineFarMoveActor( void* Level, void* Actor, const void* Dest )
{
	return ((ULevel*)Level)->FarMoveActor( (AActor*)Actor, *(const FVector*)Dest, 0, 0 );
}

static INT EnginePointReachable( void* Pawn, const void* Point )
{
	return ((APawn*)Pawn)->pointReachable( *(const FVector*)Point, 0 );
}

static void EngineSetCollisionSize( void* Actor, FLOAT Radius, FLOAT Height )
{
	((AActor*)Actor)->SetCollisionSize( Radius, Height );
}

static BYTE EngineModelFastLineCheck( void* Model, const void* End, const void* Start )
{
	return ((UModel*)Model)->FastLineCheck( *(const FVector*)End, *(const FVector*)Start );
}

static void EngineRotatorVector( const void* Rot, void* Out )
{
	// FRotator::Vector is non-const; operate on a copy.
	FRotator Rotation = *(const FRotator*)Rot;
	*(FVector*)Out = Rotation.Vector();
}

const Hp2EngineServices GHp2EngineServices =
{
	EngineAppFrand,
	EngineAppRand,
	EngineFarMoveActor,
	EnginePointReachable,
	EngineSetCollisionSize,
	EngineModelFastLineCheck,
	EngineRotatorVector,
};

static_assert( sizeof(Hp2EngineServices) == 7*sizeof(void*), "Hp2EngineServices must match hp2-engine::services::Hp2EngineServices" );
