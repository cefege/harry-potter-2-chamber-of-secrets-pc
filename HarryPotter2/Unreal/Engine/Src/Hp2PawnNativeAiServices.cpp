/*=============================================================================
	Hp2PawnNativeAiServices.cpp: Pawn AI native services called by the Rust
	engine port (crates/hp2-engine/src/pawn_native_ai.rs). Each adapter decodes
	raw pointers into the C++ types and forwards to the engine-owned
	implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PawnNativeAiServices.h"

static void NativeAiRotationVector( const void* Rot, void* Out )
{
	*(FVector*)Out = ((FRotator*)Rot)->Vector();
}

static INT NativeAiStateLatentAction( void* Actor )
{
	return ((AActor*)Actor)->GetStateFrame()->LatentAction;
}

static void NativeAiSetStateLatentAction( void* Actor, INT Action )
{
	((AActor*)Actor)->GetStateFrame()->LatentAction = Action;
}

static INT NativeAiLevelNetMode( void* Level )
{
	return ((ULevel*)Level)->GetLevelInfo()->NetMode;
}

static void* NativeAiLevelPawnList( void* Level )
{
	return ((ULevel*)Level)->GetLevelInfo()->PawnList;
}

static void NativeAiSetLevelPawnList( void* Level, void* Pawn )
{
	((ULevel*)Level)->GetLevelInfo()->PawnList = (APawn*)Pawn;
}

static INT NativeAiLevelTeamGame( void* Level )
{
	return ((ULevel*)Level)->GetLevelInfo()->Game->bTeamGame;
}

static INT NativeAiLevelActorCount( void* Level )
{
	return ((ULevel*)Level)->Actors.Num();
}

static void* NativeAiLevelActor( void* Level, INT Index )
{
	return ((ULevel*)Level)->Actors( Index );
}

static INT NativeAiViewportCount( void* Level )
{
	return ((ULevel*)Level)->Engine->Client->Viewports.Num();
}

static void* NativeAiViewportActor( void* Level, INT Index )
{
	return ((ULevel*)Level)->Engine->Client->Viewports( Index )->Actor;
}

static INT NativeAiActorNetTemporary( void* Actor )
{
	return ((AActor*)Actor)->bNetTemporary;
}

static INT NativeAiActorProjTarget( void* Actor )
{
	return ((AActor*)Actor)->bProjTarget;
}

static INT NativeAiActorIsPawn( void* Actor )
{
	return ((AActor*)Actor)->IsA( APawn::StaticClass() );
}

static INT NativeAiPawnHealth( void* Pawn )
{
	return ((APawn*)Pawn)->Health;
}

static INT NativeAiPawnBehindView( void* Pawn )
{
	return ((APlayerPawn*)Pawn)->bBehindView;
}

static INT NativeAiReplicationTeam( void* Info )
{
	return ((APlayerReplicationInfo*)Info)->Team;
}

static INT NativeAiLineOfSightTo( void* Pawn, void* Other )
{
	return ((APawn*)Pawn)->LineOfSightTo( (AActor*)Other );
}

static void NativeAiSingleLineCheck( void* Level, void* Source, void* Hit, const void* End, const void* Start, DWORD Trace, const void* Extent )
{
	((ULevel*)Level)->SingleLineCheck( *(FCheckResult*)Hit, (AActor*)Source, *(const FVector*)End, *(const FVector*)Start, Trace, *(const FVector*)Extent );
}

static void NativeAiEventLongFall( void* Pawn )
{
	((APawn*)Pawn)->eventLongFall();
}

const Hp2PawnNativeAiServices GHp2PawnNativeAiServices =
{
	NativeAiRotationVector,
	NativeAiStateLatentAction,
	NativeAiSetStateLatentAction,
	NativeAiLevelNetMode,
	NativeAiLevelPawnList,
	NativeAiSetLevelPawnList,
	NativeAiLevelTeamGame,
	NativeAiLevelActorCount,
	NativeAiLevelActor,
	NativeAiViewportCount,
	NativeAiViewportActor,
	NativeAiActorNetTemporary,
	NativeAiActorProjTarget,
	NativeAiActorIsPawn,
	NativeAiPawnHealth,
	NativeAiPawnBehindView,
	NativeAiReplicationTeam,
	NativeAiLineOfSightTo,
	NativeAiSingleLineCheck,
	NativeAiEventLongFall,
};

static_assert( sizeof(Hp2PawnNativeAiServices) == 20*sizeof(void*), "Hp2PawnNativeAiServices must match hp2-engine::pawn_native_ai::Hp2PawnNativeAiServices" );
