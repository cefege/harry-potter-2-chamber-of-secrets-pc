/*=============================================================================
	Hp2PhysicServices.h: C++ services for the Rust actor physics port.

	Consumed by crates/hp2-engine/src/physic.rs (`Hp2PhysicServices`, #[repr(C)]).
	Field order and the class/trace ids below must match the Rust side exactly.
	The Rust exports take every pointer as void*, so this header is only needed
	by the C++ translation units that pass GHp2PhysicServices to them.
=============================================================================*/

#ifndef _INC_HP2PHYSICSERVICES_H
#define _INC_HP2PHYSICSERVICES_H

class FVector;
class FRotator;
struct FCheckResult;

// Mirrors physic.rs `CLASS_*`: class tests answered by UObject::IsA.
enum Hp2PhysicClass
{
	HP2_PHYSIC_CLASS_PAWN         = 0,
	HP2_PHYSIC_CLASS_PLAYER_PAWN  = 1,
	HP2_PHYSIC_CLASS_DECORATION   = 2,
	HP2_PHYSIC_CLASS_CARCASS      = 3,
};

// Mirrors physic.rs `TRACE_*`: trace flag sets passed to ULevel::SingleLineCheck.
enum Hp2PhysicTrace
{
	HP2_PHYSIC_TRACE_VIS_BLOCKING = 0,
	HP2_PHYSIC_TRACE_ALL_BLOCKING = 1,
};

struct Hp2PhysicServices
{
	int   (*IsA)( void* Obj, int Class );
	int   (*IsProbingHitWall)( void* Actor );
	float (*AppFrand)();
	void  (*CylinderExtent)( void* Actor, FVector* Out );
	int   (*BspSurfSpecial)( const FCheckResult* Hit, float Radius );
	void  (*RotationVector)( const FRotator* Rot, FVector* Out );
	int   (*MoveActor)( void* Actor, const FVector* Delta, const FRotator* Rot, FCheckResult* Hit );
	int   (*SingleLineCheck)( void* Actor, FCheckResult* Hit, const FVector* End, const FVector* Start, int Trace, const FVector* Extent );
	void  (*SetBase)( void* Actor, void* Base );
	void  (*EventMount)( void* Actor, const FVector* Dir );
	void  (*EventHitWall)( void* Actor, const FVector* Normal, void* HitActor );
	void  (*EventLanded)( void* Actor, const FVector* Normal );
	void  (*SetPhysics)( void* Actor, int Physics, void* Floor );
	void  (*PhysWalking)( void* Actor, float RemainingTime, int Iterations );
};

extern const Hp2PhysicServices GHp2PhysicServices;

#endif
