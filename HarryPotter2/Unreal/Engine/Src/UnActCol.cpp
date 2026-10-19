/*=============================================================================
	UnLevCol.cpp: Actor list collision code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Design goal:
	To be self-contained. This collision code maintains its own collision hash
	table and doesn't know about any far-away data structures like the level BSP.

Revision history:
	* Created by Tim Sweeney

The collision hash itself lives in crates/hp2-engine/src/actcol.rs. This file
is the C++ shell: it owns the Rust handle and forwards each virtual.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2ActColServices.h"

/*-----------------------------------------------------------------------------
	FCollisionHash.
-----------------------------------------------------------------------------*/

//
// A collision hash table (C++ shell over the Rust CollisionHash).
//
class ENGINE_API FCollisionHash : public FCollisionHashBase
{
public:
	// FCollisionHashBase interface.
	FCollisionHash();
	~FCollisionHash();
	void Tick();
	void AddActor( AActor *Actor );
	void RemoveActor( AActor *Actor, bool bThorough = false );
	FCheckResult* ActorLineCheck( FMemStack& Mem, FVector End, FVector Start, FVector Extent, BYTE ExtraNodeFlags );
	FCheckResult* ActorPointCheck( FMemStack& Mem, FVector Location, FVector Extent, DWORD ExtraNodeFlags );
	FCheckResult* ActorRadiusCheck( FMemStack& Mem, FVector Location, FLOAT Radius, DWORD ExtraNodeFlags );
	FCheckResult* ActorEncroachmentCheck( FMemStack& Mem, AActor* Actor, FVector Location, FRotator Rotation, DWORD ExtraNodeFlags );
	void CheckActorNotReferenced( AActor* Actor );

private:
	void* RustHash;	// Rust CollisionHash (owned; freed in the destructor).
};

ENGINE_API FCollisionHashBase* GNewCollisionHash()
{
	guard(GNewCollisionHash);
	return new(TEXT("FCollisionHash"))FCollisionHash;
	unguard;
}

/*-----------------------------------------------------------------------------
	FCollisionHash init/exit.
-----------------------------------------------------------------------------*/

FCollisionHash::FCollisionHash()
: RustHash( hp2_rs_actcol_new( &GHp2ActColServices ) )
{
	guard(FCollisionHash::FCollisionHash);
	unguard;
}
FCollisionHash::~FCollisionHash()
{
	guard(FCollisionHash::~FCollisionHash);
	hp2_rs_actcol_free( RustHash );
	RustHash = NULL;
	unguard;
}

/*-----------------------------------------------------------------------------
	Adding and removing actors.
-----------------------------------------------------------------------------*/

void FCollisionHash::Tick()
{
	guard(FCollisionHash::Tick);
	hp2_rs_actcol_tick( RustHash );
	unguard;
}
void FCollisionHash::AddActor( AActor* Actor )
{
	guard(FCollisionHash::AddActor);
	hp2_rs_actcol_add_actor( RustHash, &GHp2ActColServices, Actor );
	unguard;
}
void FCollisionHash::RemoveActor( AActor* Actor, bool bThorough )
{
	guard(FCollisionHash::RemoveActor);
	hp2_rs_actcol_remove_actor( RustHash, &GHp2ActColServices, Actor, bThorough );
	unguard;
}

/*-----------------------------------------------------------------------------
	Checks. Results are FCheckResult list heads (null = no hits).
-----------------------------------------------------------------------------*/

FCheckResult* FCollisionHash::ActorLineCheck( FMemStack& Mem, FVector End, FVector Start, FVector Extent, BYTE ExtraNodeFlags )
{
	guard(FCollisionHash::ActorLineCheck);
	return (FCheckResult*)hp2_rs_actcol_actor_line_check( RustHash, &GHp2ActColServices, &Mem, &End, &Start, &Extent, ExtraNodeFlags );
	unguard;
}
FCheckResult* FCollisionHash::ActorPointCheck( FMemStack& Mem, FVector Location, FVector Extent, DWORD ExtraNodeFlags )
{
	guard(FCollisionHash::ActorPointCheck);
	return (FCheckResult*)hp2_rs_actcol_actor_point_check( RustHash, &GHp2ActColServices, &Location, &Extent );
	unguard;
}
FCheckResult* FCollisionHash::ActorRadiusCheck( FMemStack& Mem, FVector Location, FLOAT Radius, DWORD ExtraNodeFlags )
{
	guard(FCollisionHash::ActorRadiusCheck);
	return (FCheckResult*)hp2_rs_actcol_actor_radius_check( RustHash, &GHp2ActColServices, &Location, Radius );
	unguard;
}
FCheckResult* FCollisionHash::ActorEncroachmentCheck( FMemStack& Mem, AActor* Actor, FVector Location, FRotator Rotation, DWORD ExtraNodeFlags )
{
	guard(FCollisionHash::ActorEncroachmentCheck);
	return (FCheckResult*)hp2_rs_actcol_actor_encroachment_check( RustHash, &GHp2ActColServices, Actor, &Location, &Rotation );
	unguard;
}

/*-----------------------------------------------------------------------------
	Debugging.
-----------------------------------------------------------------------------*/

void FCollisionHash::CheckActorNotReferenced( AActor* Actor )
{
	guard(FCollisionHash::CheckActorNotReferenced);
	// The original body was dead code (`if( 0 && DO_GUARD_SLOW && !GIsEditor )`);
	// nothing is checked here.
	unguardf(( TEXT("(%s)"), Actor->GetFullName() ));
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
