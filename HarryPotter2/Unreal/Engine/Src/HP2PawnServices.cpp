/*=============================================================================
	HP2PawnServices.cpp: Pawn move-primitive services called by the Rust engine
	port (crates/hp2-engine/src/pawn.rs). Each adapter decodes raw pointers into
	the C++ types and forwards to the engine-owned implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"
#include "HP2PawnServices.h"
#include "Hp2PhysicServices.h"

static INT PawnMoveActor( void* Level, void* Actor, const void* Delta, const void* Rot, void* Hit )
{
	return ((ULevel*)Level)->MoveActor( (AActor*)Actor, *(const FVector*)Delta, *(const FRotator*)Rot, *(FCheckResult*)Hit, 1, 1 );
}

static INT PawnFarMoveActor( void* Level, void* Actor, const void* Dest )
{
	return ((ULevel*)Level)->FarMoveActor( (AActor*)Actor, *(const FVector*)Dest, 1, 1 );
}

// APawn::findWaterLine is a private forward to this same export.
static void PawnFindWaterLine( void* Pawn, const void* Start, void* End )
{
	hp2_rs_engine_physic_find_water_line( Pawn, Start, End, &GHp2PhysicServices );
}

const Hp2PawnServices GHp2PawnServices =
{
	PawnMoveActor,
	PawnFarMoveActor,
	PawnFindWaterLine,
};

static_assert( sizeof(Hp2PawnServices) == 3*sizeof(void*), "Hp2PawnServices must match hp2-engine::pawn::Hp2PawnServices" );
