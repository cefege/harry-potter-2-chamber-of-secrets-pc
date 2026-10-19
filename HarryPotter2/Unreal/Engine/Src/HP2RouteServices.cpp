/*=============================================================================
	HP2RouteServices.cpp: Route-finding services called by the Rust engine port
	(crates/hp2-engine/src/route.rs). Each adapter decodes raw pointers into the
	C++ types and forwards to the engine-owned implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "HP2RouteServices.h"

static FLOAT RouteAppFrand()
{
	return appFrand();
}

static INT RouteFarMoveActor( void* Level, void* Actor, const void* Dest, INT Test, INT NoCheck )
{
	return ((ULevel*)Level)->FarMoveActor( (AActor*)Actor, *(const FVector*)Dest, Test, NoCheck );
}

static INT RoutePointReachable( void* Pawn, const void* Point, INT KnowVisible )
{
	return ((APawn*)Pawn)->pointReachable( *(const FVector*)Point, KnowVisible );
}

static INT RouteActorReachable( void* Pawn, void* Other, INT KnowVisible )
{
	return ((APawn*)Pawn)->actorReachable( (AActor*)Other, KnowVisible );
}

static void RouteGoalJumpLanding( void* Goal, void* Landing )
{
	AActor* G = (AActor*)Goal;
	((APawn*)Goal)->jumpLanding( G->Velocity, *(FVector*)Landing );
}

static BYTE RouteModelFastLineCheck( void* Model, const void* End, const void* Start )
{
	return ((UModel*)Model)->FastLineCheck( *(const FVector*)End, *(const FVector*)Start );
}

static void* RouteSingleLineHitActor( void* Level, void* Source, const void* End, const void* Start )
{
	FCheckResult Hit( 1.f );
	((ULevel*)Level)->SingleLineCheck( Hit, (AActor*)Source, *(const FVector*)End, *(const FVector*)Start, TRACE_VisBlocking );
	return Hit.Actor;
}

static void* RouteNavigationPointList( void* Level )
{
	return ((ULevel*)Level)->GetLevelInfo()->NavigationPointList;
}

static INT RouteActorIsA( void* Actor, INT Kind )
{
	AActor* A = (AActor*)Actor;
	switch ( Kind )
	{
		case 0: return A->IsA( ANavigationPoint::StaticClass() );
		case 1: return A->IsA( APawn::StaticClass() );
		case 2: return A->IsA( AInventorySpot::StaticClass() );
		case 3: return A->IsA( AMover::StaticClass() );
	}
	return 0;
}

static INT RouteMoverPlayerOnly( void* Mover )
{
	return ((AMover*)Mover)->bPlayerOnly;
}

static INT RouteEventSpecialCost( void* Node, void* Seeker )
{
	return ((ANavigationPoint*)Node)->eventSpecialCost( (APawn*)Seeker );
}

static INT RouteItemIsProbingTouch( void* Item )
{
	return ((AInventory*)Item)->IsProbing( NAME_Touch );
}

static FLOAT RouteItemBotDesirability( void* Item, void* Bot )
{
	return ((AInventory*)Item)->eventBotDesireability( (APawn*)Bot );
}

static INT RouteActorNameIndex( void* Actor )
{
	return ((AActor*)Actor)->GetFName().GetIndex();
}

static void* RouteActorClass( void* Actor )
{
	return ((AActor*)Actor)->GetClass();
}

static void RouteLogActor( INT Kind, void* Actor )
{
	AActor* A = (AActor*)Actor;
	switch ( Kind )
	{
		case 0: debugf( TEXT("startname is %s"), A->GetName() ); break;
		case 1: debugf( TEXT("endname is %s"), A->GetName() ); break;
		case 2: debugf( TEXT("Breadth path list overflow from %s"), A->GetName() ); break;
		case 3: debugf( NAME_DevPath, TEXT("1000 navigation nodes searched from %s!"), A->GetName() ); break;
	}
}

const Hp2RouteServices GHp2RouteServices =
{
	RouteAppFrand,
	RouteFarMoveActor,
	RoutePointReachable,
	RouteActorReachable,
	RouteGoalJumpLanding,
	RouteModelFastLineCheck,
	RouteSingleLineHitActor,
	RouteNavigationPointList,
	RouteActorIsA,
	RouteMoverPlayerOnly,
	RouteEventSpecialCost,
	RouteItemIsProbingTouch,
	RouteItemBotDesirability,
	RouteActorNameIndex,
	RouteActorClass,
	RouteLogActor,
};

static_assert( sizeof(Hp2RouteServices) == 16*sizeof(void*), "Hp2RouteServices must match hp2-engine::route::Hp2RouteServices" );
