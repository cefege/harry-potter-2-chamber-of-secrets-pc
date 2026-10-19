/*=============================================================================
	Hp2PawnSenseServices.cpp: Pawn sense services called by the Rust engine
	port (crates/hp2-engine/src/pawn_sense.rs). Each adapter decodes raw
	pointers into the C++ types and forwards to the engine-owned implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PawnSenseServices.h"

static INT SenseFastLineCheck( void* Level, const void* End, const void* Start )
{
	return ((ULevel*)Level)->Model->FastLineCheck( *(const FVector*)End, *(const FVector*)Start );
}

static void SenseSingleLineCheck( void* Level, void* Source, void* Hit, const void* End, const void* Start, DWORD Trace, const void* Extent )
{
	((ULevel*)Level)->SingleLineCheck( *(FCheckResult*)Hit, (AActor*)Source, *(const FVector*)End, *(const FVector*)Start, Trace, *(const FVector*)Extent );
}

static FLOAT SenseAppFrand( void )
{
	return appFrand();
}

static void SenseSetPhysics( void* Actor, BYTE Phys )
{
	((AActor*)Actor)->setPhysics( Phys );
}

static void SenseGetCylinderExtent( void* Actor, void* Out )
{
	*(FVector*)Out = ((AActor*)Actor)->GetCylinderExtent();
}

static INT SenseIsProbing( void* Object, DWORD Name )
{
	UObject* Obj = (UObject*)Object;
	switch( Name )
	{
		case 0: return Obj->IsProbing( NAME_HearNoise );
		case 1: return Obj->IsProbing( NAME_SpecialHandling );
	}
	return 0;
}

static void SenseEventHearNoise( void* Pawn, FLOAT Loudness, void* Noise )
{
	((APawn*)Pawn)->eventHearNoise( Loudness, (AActor*)Noise );
}

static void SenseEventEnemyNotVisible( void* Pawn )
{
	((APawn*)Pawn)->eventEnemyNotVisible();
}

static void SenseEventTouch( void* Actor, void* Other )
{
	((AActor*)Actor)->eventTouch( (AActor*)Other );
}

static void SenseSafeNormal( const void* V, void* Out )
{
	*(FVector*)Out = ((const FVector*)V)->SafeNormal();
}

static void SenseVectorRotation( const void* V, void* Out )
{
	*(FRotator*)Out = ((FVector*)V)->Rotation();
}

static void SenseRotationVector( const void* Rot, void* Out )
{
	*(FVector*)Out = ((FRotator*)Rot)->Vector();
}

static INT SenseIsA( void* Object, DWORD Kind )
{
	UObject* Obj = (UObject*)Object;
	switch( Kind )
	{
		case 1: return Obj->IsA( APawn::StaticClass() );
		case 2: return Obj->IsA( APlayerPawn::StaticClass() );
		case 3: return Obj->IsA( AInventory::StaticClass() );
	}
	return 0;
}

static INT SenseIsAClassOf( void* Object, void* Other )
{
	return ((UObject*)Object)->IsA( ((UObject*)Other)->GetClass() );
}

static void* SenseLevelPawnHead( void* Level )
{
	return ((ULevel*)Level)->GetLevelInfo()->PawnList;
}

static INT SenseLevelTeamGame( void* Level )
{
	return ((ULevel*)Level)->GetLevelInfo()->Game->bTeamGame;
}

static FLOAT SenseLevelTime( void* Level )
{
	return ((ULevel*)Level)->TimeSeconds.GetFloat();
}

static INT SenseTeamsDiffer( void* PriA, void* PriB )
{
	return ((APlayerReplicationInfo*)PriA)->Team != ((APlayerReplicationInfo*)PriB)->Team;
}

static FLOAT SenseZoneDefaultGravityZ( void* Zone )
{
	return ((AZoneInfo*)((UObject*)Zone)->GetClass()->GetDefaultObject())->ZoneGravity.Z;
}

static INT SenseActorReachable( void* Pawn, void* Other, INT KnowVisible )
{
	return ((APawn*)Pawn)->actorReachable( (AActor*)Other, KnowVisible );
}

static INT SenseFindPathToward( void* Pawn, void* Goal, INT Single, void** BestPath, INT Clear )
{
	return ((APawn*)Pawn)->findPathToward( (AActor*)Goal, Single, *(AActor**)BestPath, Clear );
}

static void* SenseEventSpecialHandling( void* Actor, void* Pawn )
{
	return ((AActor*)Actor)->eventSpecialHandling( (APawn*)Pawn );
}

const Hp2PawnSenseServices GHp2PawnSenseServices =
{
	SenseFastLineCheck,
	SenseSingleLineCheck,
	SenseAppFrand,
	SenseSetPhysics,
	SenseGetCylinderExtent,
	SenseIsProbing,
	SenseEventHearNoise,
	SenseEventEnemyNotVisible,
	SenseEventTouch,
	SenseSafeNormal,
	SenseVectorRotation,
	SenseRotationVector,
	SenseIsA,
	SenseIsAClassOf,
	SenseLevelPawnHead,
	SenseLevelTeamGame,
	SenseLevelTime,
	SenseTeamsDiffer,
	SenseZoneDefaultGravityZ,
	SenseActorReachable,
	SenseFindPathToward,
	SenseEventSpecialHandling,
};

static_assert( sizeof(Hp2PawnSenseServices) == 22*sizeof(void*), "Hp2PawnSenseServices must match hp2-engine::pawn_sense::Hp2PawnSenseServices" );
