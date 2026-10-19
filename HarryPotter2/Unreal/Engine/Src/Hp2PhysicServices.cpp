/*=============================================================================
	Hp2PhysicServices.cpp: Thunks the Rust actor physics port calls back into.
	Each thunk performs one of the original engine operations unchanged.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PhysicServices.h"
#include "hp2_rs.h"
#include "UnMesh.h"

static UClass* PhysicClassOf( int Class )
{
	switch( Class )
	{
		case HP2_PHYSIC_CLASS_PAWN:        return APawn::StaticClass();
		case HP2_PHYSIC_CLASS_PLAYER_PAWN: return APlayerPawn::StaticClass();
		case HP2_PHYSIC_CLASS_DECORATION:  return ADecoration::StaticClass();
		case HP2_PHYSIC_CLASS_CARCASS:     return ACarcass::StaticClass();
		case HP2_PHYSIC_CLASS_PROJECTILE:  return AProjectile::StaticClass();
		case HP2_PHYSIC_CLASS_MOVER:       return AMover::StaticClass();
		case HP2_PHYSIC_CLASS_INVENTORY:   return AInventory::StaticClass();
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

static int PhysSingleLineCheckNoSource( void* Actor, FCheckResult* Hit, const FVector* End, const FVector* Start, int Trace, const FVector* Extent )
{
	DWORD TraceFlags = (Trace == HP2_PHYSIC_TRACE_ALL_BLOCKING) ? TRACE_AllBlocking : TRACE_VisBlocking;
	return ((AActor*)Actor)->GetLevel()->SingleLineCheck( *Hit, NULL, *End, *Start, TraceFlags, *Extent );
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


static void PhysPhysTrailer( void* Actor, float DeltaTime )
{
	hp2_rs_engine_physic_phys_trailer( Actor, DeltaTime, &GHp2PhysicServices );
}

static void PhysPhysMovingBrush( void* Actor, float DeltaTime )
{
	((AActor*)Actor)->physMovingBrush( DeltaTime );
}

static void PhysEventPostTouch( void* Touched, void* Toucher )
{
	((AActor*)Touched)->eventPostTouch( (AActor*)Toucher );
}

static int PhysIsProbingEndedRotation( void* Actor )
{
	return ((UObject*)Actor)->IsProbing( NAME_EndedRotation );
}

static void PhysEventEndedRotation( void* Actor )
{
	((AActor*)Actor)->eventEndedRotation();
}

static float PhysSinTab( int I )
{
	return GMath.SinTab( I );
}

static float PhysCosTab( int I )
{
	return GMath.CosTab( I );
}

static void* PhysPointRegionZone( void* Actor, const FVector* Location )
{
	AActor* A = (AActor*)Actor;
	return A->GetLevel()->Model->PointRegion( A->Level, *Location ).Zone;
}

static void PhysDebugfFlewOut( void* Actor )
{
	debugf( TEXT("%s flew out of the world!"), ((AActor*)Actor)->GetName() );
}

static void PhysDestroyActor( void* Actor )
{
	((AActor*)Actor)->GetLevel()->DestroyActor( (AActor*)Actor );
}

static void PhysEventFalling( void* Actor )
{
	((AActor*)Actor)->eventFalling();
}

static float PhysProjectileMaxSpeed( void* Actor )
{
	return ((AProjectile*)Actor)->MaxSpeed;
}

static void PhysLogFellOutOfWorld( void* Actor )
{
	debugf( TEXT("%s fell out of the world!"), ((AActor*)Actor)->GetName() );
}

static void PhysEventFellOutOfWorld( void* Actor )
{
	((AActor*)Actor)->eventFellOutOfWorld();
}
static int PhysIsProbingMayFall( void* Actor )
{
	return ((UObject*)Actor)->IsProbing( NAME_MayFall );
}

static void PhysEventMayFall( void* Actor )
{
	((APawn*)Actor)->eventMayFall();
}

static void PhysEventDoJump( void* Actor, float Value )
{
	((APlayerPawn*)Actor)->eventDoJump( Value );
}

static void PhysFarMoveActor( void* Actor, const FVector* Location )
{
	((AActor*)Actor)->GetLevel()->FarMoveActor( (AActor*)Actor, *Location, 0, 0 );
}

static void PhysTrailerState( void* Actor, void* Out )
{
	AActor* A = (AActor*)Actor;
	Hp2PhysicTrailerState* S = (Hp2PhysicTrailerState*)Out;
	S->Owner = A->Owner;
	S->Mesh = A->Owner ? A->Owner->Mesh : NULL;
	S->PrePivot[0] = A->PrePivot.X;
	S->PrePivot[1] = A->PrePivot.Y;
	S->PrePivot[2] = A->PrePivot.Z;
	S->Mass = A->Mass;
	S->DrawType = A->DrawType;
	S->AnimBone = A->AnimBone;
	S->Flags = (A->bTrailerPrePivot ? 1u : 0u)
		| (A->bTrailerSameRotation ? 2u : 0u)
		| (A->bBlockActors ? 4u : 0u)
		| (A->bCollideWorld ? 8u : 0u);
}

static void PhysMeshBoneCoords( void* Mesh, void* Owner, int Bone, void* Out )
{
	*(FCoords*)Out = ((UMesh*)Mesh)->GetBoneCoords( (AActor*)Owner, Bone );
}

static void PhysTrailerFarMoveActor( void* Actor, const FVector* Location )
{
	((AActor*)Actor)->GetLevel()->FarMoveActor( (AActor*)Actor, *Location, 0, 1 );
}

static void PhysDebugfFellOut( void* Actor )
{
	debugf( TEXT("%s fell out of the world!"), ((AActor*)Actor)->GetName() );
}


static void PhysRotationFromVector( const FVector* Vector, FRotator* Out )
{
	FVector V = *Vector;
	*Out = V.Rotation();
}

static void* PhysFindViewTarget( void* Actor, const BYTE* Tag )
{
	AActor* A = (AActor*)Actor;
	FName Wanted = *(const FName*)Tag;
	for( INT I=0; I<A->GetLevel()->Actors.Num(); I++ )
	{
		AActor* Candidate = A->GetLevel()->Actors(I);
		if( Candidate && Candidate->Tag == Wanted )
			return Candidate;
	}
	return NULL;
}

static void PhysEventUpdateCamera( void* Manager, float OldAlpha )
{
	((AInterpolationManager*)Manager)->eventUpdateCamera( OldAlpha );
}

static void PhysEventInterpolateEnd( void* Point, void* Manager, int Forward )
{
	((AInterpolationPoint*)Point)->eventInterpolateEnd( (AInterpolationManager*)Manager, Forward );
}

static void PhysEventFinishedInterpolation( void* Manager )
{
	((AInterpolationManager*)Manager)->eventFinishedInterpolation( NULL );
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
	PhysPointRegionZone,
	PhysDebugfFlewOut,
	PhysDestroyActor,
	PhysEventFalling,
	PhysProjectileMaxSpeed,
	PhysLogFellOutOfWorld,
	PhysEventFellOutOfWorld,
	PhysRotationFromVector,
	PhysFindViewTarget,
	PhysEventUpdateCamera,
	PhysEventInterpolateEnd,
	PhysEventFinishedInterpolation,
	PhysPhysTrailer,
	PhysPhysMovingBrush,
	PhysEventPostTouch,
	PhysIsProbingEndedRotation,
	PhysEventEndedRotation,
	PhysSinTab,
	PhysCosTab,
	PhysIsProbingMayFall,
	PhysEventMayFall,
	PhysEventDoJump,
	PhysFarMoveActor,
	PhysDebugfFellOut,
	PhysSingleLineCheckNoSource,
	PhysTrailerState,
	PhysMeshBoneCoords,
	PhysTrailerFarMoveActor,
};
