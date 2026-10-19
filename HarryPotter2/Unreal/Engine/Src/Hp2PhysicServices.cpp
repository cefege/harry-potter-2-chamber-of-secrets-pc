/*=============================================================================
	Hp2PhysicServices.cpp: Thunks the Rust actor physics port calls back into.
	Each thunk performs one of the original engine operations unchanged.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PhysicServices.h"

static UClass* PhysicClassOf( int Class )
{
	switch( Class )
	{
		case HP2_PHYSIC_CLASS_PAWN:        return APawn::StaticClass();
		case HP2_PHYSIC_CLASS_PLAYER_PAWN: return APlayerPawn::StaticClass();
		case HP2_PHYSIC_CLASS_DECORATION:  return ADecoration::StaticClass();
		case HP2_PHYSIC_CLASS_CARCASS:     return ACarcass::StaticClass();
		default:                           appErrorf( TEXT("Hp2 physic: bad class id %d"), Class ); return NULL;
	}
}

static int PhysIsA( void* Obj, int Class )
{
	return ((UObject*)Obj)->IsA( PhysicClassOf(Class) );
}

static int PhysIsProbingHitWall( void* Actor )
{
	return ((UObject*)Actor)->IsProbing( NAME_HitWall );
}

static float PhysAppFrand()
{
	return appFrand();
}

static void PhysCylinderExtent( void* Actor, FVector* Out )
{
	*Out = ((AActor*)Actor)->GetCylinderExtent();
}

static int PhysBspSurfSpecial( const FCheckResult* Hit, float Radius )
{
	FBspSurf* Surf = Hit->BspSurf( Radius );
	return Surf && (Surf->PolyFlags & PF_SpecialPoly);
}

static void PhysRotationVector( const FRotator* Rot, FVector* Out )
{
	FRotator Copy = *Rot;
	*Out = Copy.Vector();
}

static int PhysMoveActor( void* Actor, const FVector* Delta, const FRotator* Rot, FCheckResult* Hit )
{
	return ((AActor*)Actor)->GetLevel()->MoveActor( (AActor*)Actor, *Delta, *Rot, *Hit );
}

static int PhysSingleLineCheck( void* Actor, FCheckResult* Hit, const FVector* End, const FVector* Start, int Trace, const FVector* Extent )
{
	DWORD TraceFlags = (Trace == HP2_PHYSIC_TRACE_ALL_BLOCKING) ? TRACE_AllBlocking : TRACE_VisBlocking;
	return ((AActor*)Actor)->GetLevel()->SingleLineCheck( *Hit, (AActor*)Actor, *End, *Start, TraceFlags, *Extent );
}

static void PhysSetBase( void* Actor, void* Base )
{
	((AActor*)Actor)->SetBase( (AActor*)Base );
}

static void PhysEventMount( void* Actor, const FVector* Dir )
{
	((APawn*)Actor)->eventMount( *Dir );
}

static void PhysEventHitWall( void* Actor, const FVector* Normal, void* HitActor )
{
	((AActor*)Actor)->eventHitWall( *Normal, (AActor*)HitActor );
}

static void PhysEventLanded( void* Actor, const FVector* Normal )
{
	((AActor*)Actor)->eventLanded( *Normal );
}

static void PhysSetPhysics( void* Actor, int Physics, void* Floor )
{
	((AActor*)Actor)->setPhysics( (BYTE)Physics, (AActor*)Floor );
}

static void PhysPhysWalking( void* Actor, float RemainingTime, int Iterations )
{
	((APawn*)Actor)->physWalking( RemainingTime, Iterations );
}

const Hp2PhysicServices GHp2PhysicServices =
{
	PhysIsA,
	PhysIsProbingHitWall,
	PhysAppFrand,
	PhysCylinderExtent,
	PhysBspSurfSpecial,
	PhysRotationVector,
	PhysMoveActor,
	PhysSingleLineCheck,
	PhysSetBase,
	PhysEventMount,
	PhysEventHitWall,
	PhysEventLanded,
	PhysSetPhysics,
	PhysPhysWalking,
};
