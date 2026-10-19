/*=============================================================================
	Hp2ActColServices.cpp: Thunks the Rust FCollisionHash port calls back into.
	Each thunk is a one-line forward to the original C++ call it replaces.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2ActColServices.h"

static int ActColAppRand( void )
{
	return appRand();
}

static void ActColActorCollisionBoundingBox( void* Actor, void* OutBox )
{
	AActor* A = (AActor*)Actor;
	*(FBox*)OutBox = A->GetPrimitive()->GetCollisionBoundingBox( A, true );
}

static void ActColActorCylinderExtent( const void* Actor, void* OutExtent )
{
	*(FVector*)OutExtent = ((const AActor*)Actor)->GetCylinderExtent();
}

static int ActColActorIsMovingBrush( const void* Actor )
{
	return ((const AActor*)Actor)->IsMovingBrush();
}

static int ActColPrimitivePointCheck( void* Owner, void* Res, const void* Location, const void* Extent )
{
	AActor* A = (AActor*)Owner;
	return A->GetPrimitive()->PointCheck( *(FCheckResult*)Res, A, *(const FVector*)Location, *(const FVector*)Extent, 0 );
}

static int ActColPrimitiveLineCheck( void* Actor, void* Res, const void* End, const void* Start, const void* Size, unsigned int Flags )
{
	AActor* A = (AActor*)Actor;
	return A->GetPrimitive()->LineCheck( *(FCheckResult*)Res, A, *(const FVector*)End, *(const FVector*)Start, *(const FVector*)Size, Flags );
}

static void* ActColAllocCheckResult( void* Mem )
{
	// A null Mem means GMem, matching the original `new(GMem)FCheckResult`.
	if( !Mem )
		return new(GMem)FCheckResult;
	return new(*(FMemStack*)Mem)FCheckResult;
}

static void ActColDebugfMovedWithoutHashing( const void* Actor )
{
	debugf( TEXT("%s moved without proper hashing"), ((const AActor*)Actor)->GetFullName() );
}

static void ActColCheckFailed( const char* Expr, const char* File, int Line )
{
	appFailAssert( Expr, File, Line );
}

const Hp2ActColServices GHp2ActColServices =
{
	ActColAppRand,
	ActColActorCollisionBoundingBox,
	ActColActorCylinderExtent,
	ActColActorIsMovingBrush,
	ActColPrimitivePointCheck,
	ActColPrimitiveLineCheck,
	ActColAllocCheckResult,
	ActColDebugfMovedWithoutHashing,
	ActColCheckFailed,
};
