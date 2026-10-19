/*=============================================================================
	Hp2PawnReachServices.cpp: Pawn reachability services called by the Rust
	engine port (crates/hp2-engine/src/pawn_reach.rs). Each adapter decodes raw
	pointers into the C++ types and forwards to the engine-owned implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "HP2PawnServices.h"
#include "Hp2PawnReachServices.h"

static INT ReachFarMoveActorFlags( void* Level, void* Actor, const void* Dest, INT Test, INT NoCheck )
{
	return ((ULevel*)Level)->FarMoveActor( (AActor*)Actor, *(const FVector*)Dest, Test, NoCheck );
}

static void ReachSingleLineCheck( void* Level, void* Source, void* Hit, const void* End, const void* Start )
{
	((ULevel*)Level)->SingleLineCheck( *(FCheckResult*)Hit, (AActor*)Source, *(const FVector*)End, *(const FVector*)Start, TRACE_VisBlocking );
}

static void ReachPointRegion( void* Level, void* Actor, const void* Point, void* Out )
{
	*(FPointRegion*)Out = ((ULevel*)Level)->Model->PointRegion( ((AActor*)Actor)->Level, *(const FVector*)Point );
}

static INT ReachFastLineCheck( void* Level, const void* End, const void* Start )
{
	return ((ULevel*)Level)->Model->FastLineCheck( *(const FVector*)End, *(const FVector*)Start );
}

static INT ReachIsA( void* Object, INT Kind )
{
	UObject* Obj = (UObject*)Object;
	switch( Kind )
	{
		case 0: return Obj->IsA( APawn::StaticClass() );
		case 1: return Obj->IsA( AInventory::StaticClass() );
		case 2: return Obj->IsA( ATrigger::StaticClass() );
		case 3: return Obj->IsA( AWarpZoneMarker::StaticClass() );
	}
	return 0;
}

static INT ReachIsEditor( void )
{
	return GIsEditor;
}

const Hp2PawnReachServices GHp2PawnReachServices =
{
	GHp2PawnServices,
	ReachFarMoveActorFlags,
	ReachSingleLineCheck,
	ReachPointRegion,
	ReachFastLineCheck,
	ReachIsA,
	ReachIsEditor,
};

static_assert( sizeof(Hp2PawnReachServices) == 9*sizeof(void*), "Hp2PawnReachServices must match hp2-engine::pawn_reach::Hp2PawnReachServices" );
