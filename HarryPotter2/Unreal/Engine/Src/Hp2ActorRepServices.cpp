/*=============================================================================
	Hp2ActorRepServices.cpp: AActor replication services called by the Rust
	engine port (crates/hp2-engine/src/actor_rep.rs). Each adapter wraps one
	engine call with the exact C signature mirrored in Rust.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "Hp2ActorRepServices.h"

// Maps the kind codes from hp2_rs.h to the class each DOREP site names.
static UClass* Hp2ActorRepClass( uint32_t Kind )
{
	switch( Kind )
	{
		case 0:  return AActor::StaticClass();
		case 1:  return APawn::StaticClass();
		case 2:  return APlayerPawn::StaticClass();
		case 3:  return AMover::StaticClass();
		case 4:  return AZoneInfo::StaticClass();
		case 5:  return ALevelInfo::StaticClass();
		case 6:  return APlayerReplicationInfo::StaticClass();
		case 7:  return AGameReplicationInfo::StaticClass();
		case 8:  return AInventory::StaticClass();
		case 9:  return AWeapon::StaticClass();
		case 10: return AAmmo::StaticClass();
		case 11: return APickup::StaticClass();
	}
	appErrorf( TEXT("Hp2ActorRepClass: unknown kind %d"), (INT)Kind );
	return NULL;
}

static uint32_t Hp2ActorRepStaticClassFlags( uint32_t Kind )
{
	return Hp2ActorRepClass( Kind )->ClassFlags;
}

static uint32_t Hp2ActorRepFindRepIndex( uint32_t Kind, const char* Name )
{
	INT Len = 0;
	while( Name[Len] )
	{
		Len++;
	}
	check( Len < 256 );
	TCHAR Wide[256];
	for( INT i=0; i<=Len; i++ )
	{
		Wide[i] = (TCHAR)(unsigned char)Name[i];
	}
	return FindObjectChecked<UProperty>( Hp2ActorRepClass( Kind ), Wide )->RepIndex;
}

static int32_t Hp2ActorRepCanSerializeObject( void* Map, void* Obj )
{
	return ((UPackageMap*)Map)->CanSerializeObject( (UObject*)Obj );
}

static int32_t Hp2ActorRepFStringNeq( const void* A, const void* B )
{
	return *(const FString*)A != *(const FString*)B;
}

static int32_t Hp2ActorRepObjectIsA( const void* Obj, uint32_t Kind )
{
	return ((UObject*)Obj)->IsA( Hp2ActorRepClass( Kind ) );
}

static const void* Hp2ActorRepDefaultActorOf( const void* Actor )
{
	return ((AActor*)Actor)->GetClass()->GetDefaultActor();
}

const Hp2ActorRepServices GHp2ActorRepServices =
{
	Hp2ActorRepStaticClassFlags,
	Hp2ActorRepFindRepIndex,
	Hp2ActorRepCanSerializeObject,
	Hp2ActorRepFStringNeq,
	Hp2ActorRepObjectIsA,
	Hp2ActorRepDefaultActorOf,
};
