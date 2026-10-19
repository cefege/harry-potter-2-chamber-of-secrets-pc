/*=============================================================================
	Hp2PawnNativeMoveServices.cpp: Pawn movement services called by the Rust
	engine port (crates/hp2-engine/src/pawn_native_move.rs). Each adapter decodes
	raw pointers into the C++ types and forwards to the engine-owned
	implementation.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PawnNativeMoveServices.h"

static INT NativeMoveIsA( void* Object, DWORD Kind )
{
	UObject* Obj = (UObject*)Object;
	switch( Kind )
	{
		case 0: return Obj->IsA( APawn::StaticClass() );
		case 1: return Obj->IsA( AInventory::StaticClass() );
		case 2: return Obj->IsA( ANavigationPoint::StaticClass() );
	}
	return 0;
}

static INT NativeMoveIsProbing( void* Object, DWORD Name )
{
	UObject* Obj = (UObject*)Object;
	switch( Name )
	{
		case 0: return Obj->IsProbing( NAME_SpecialHandling );
		case 1: return Obj->IsProbing( NAME_SeePlayer );
	}
	return 0;
}

static INT NativeMoveNetMode( void* Actor )
{
	return ((AActor*)Actor)->GetLevel()->GetLevelInfo()->NetMode;
}

static INT NativeMoveReachSpecCount( void* Pawn )
{
	return ((APawn*)Pawn)->GetLevel()->ReachSpecs.Num();
}

static void* NativeMoveLevelPawnList( void* Pawn )
{
	return ((APawn*)Pawn)->GetLevel()->GetLevelInfo()->PawnList;
}

static void* NativeMoveLevelNavList( void* Pawn )
{
	return ((APawn*)Pawn)->GetLevel()->GetLevelInfo()->NavigationPointList;
}

static INT NativeMoveActorReachable( void* Pawn, void* Actor )
{
	return ((APawn*)Pawn)->actorReachable( (AActor*)Actor );
}

static INT NativeMoveCanMoveTo( void* Pawn, void* Anchor, void* Dest )
{
	return ((APawn*)Pawn)->CanMoveTo( (AActor*)Anchor, (AActor*)Dest );
}

static INT NativeMoveFindPathTo( void* Pawn, const void* Point, INT Single, INT Clear, void** BestPath )
{
	AActor* NewPath = NULL;
	INT Found = ((APawn*)Pawn)->findPathTo( *(const FVector*)Point, Single, NewPath, Clear );
	*BestPath = NewPath;
	return Found;
}

static INT NativeMoveFindPathToward( void* Pawn, void* Goal, INT Single, INT Clear, void** BestPath )
{
	AActor* NewPath = NULL;
	INT Found = ((APawn*)Pawn)->findPathToward( (AActor*)Goal, Single, NewPath, Clear );
	*BestPath = NewPath;
	return Found;
}

static INT NativeMoveFindRandomDest( void* Pawn, void** BestPath )
{
	AActor* NewPath = NULL;
	INT Found = ((APawn*)Pawn)->findRandomDest( NewPath );
	*BestPath = NewPath;
	return Found;
}

static void NativeMoveClearPaths( void* Pawn )
{
	((APawn*)Pawn)->clearPaths();
}

static void NativeMoveHandleSpecial( void* Pawn, void** BestPath )
{
	AActor* Best = (AActor*)*BestPath;
	((APawn*)Pawn)->HandleSpecial( Best );
	*BestPath = Best;
}

static void NativeMoveSetMoveTimer( void* Pawn, FLOAT MoveSize )
{
	((APawn*)Pawn)->setMoveTimer( MoveSize );
}

static INT NativeMoveRotateToward( void* Pawn, const void* Focal )
{
	return ((APawn*)Pawn)->rotateToward( *(const FVector*)Focal );
}

static INT NativeMoveMoveToward( void* Pawn, const void* Dest )
{
	return ((APawn*)Pawn)->moveToward( *(const FVector*)Dest );
}

static void NativeMoveSetLatentAction( void* Pawn, INT Latent )
{
	((APawn*)Pawn)->GetStateFrame()->LatentAction = Latent;
}

static void NativeMoveEventAlterDestination( void* Pawn )
{
	((APawn*)Pawn)->eventAlterDestination();
}

static void NativeMoveEventSeePlayer( void* Pawn, void* Seer )
{
	((APawn*)Pawn)->eventSeePlayer( (AActor*)Seer );
}

static DWORD NativeMoveLineOfSightTo( void* Pawn, void* Other, INT ShowSelf )
{
	return ((APawn*)Pawn)->LineOfSightTo( (AActor*)Other, ShowSelf );
}

static void NativeMoveCheckNoiseHearing( void* Actor, FLOAT Loudness )
{
	((AActor*)Actor)->CheckNoiseHearing( Loudness );
}

static void NativeMoveRotationVector( void* Actor, void* Out )
{
	*(FVector*)Out = ((AActor*)Actor)->Rotation.Vector();
}

const Hp2PawnNativeMoveServices GHp2PawnNativeMoveServices =
{
	NativeMoveIsA,
	NativeMoveIsProbing,
	NativeMoveNetMode,
	NativeMoveReachSpecCount,
	NativeMoveLevelPawnList,
	NativeMoveLevelNavList,
	NativeMoveActorReachable,
	NativeMoveCanMoveTo,
	NativeMoveFindPathTo,
	NativeMoveFindPathToward,
	NativeMoveFindRandomDest,
	NativeMoveClearPaths,
	NativeMoveHandleSpecial,
	NativeMoveSetMoveTimer,
	NativeMoveRotateToward,
	NativeMoveMoveToward,
	NativeMoveSetLatentAction,
	NativeMoveEventAlterDestination,
	NativeMoveEventSeePlayer,
	NativeMoveLineOfSightTo,
	NativeMoveCheckNoiseHearing,
	NativeMoveRotationVector,
};
