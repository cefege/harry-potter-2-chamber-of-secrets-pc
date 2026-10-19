//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999
//
//  File:       UnParticleList.cpp
//
//  Contents:   Implementation of UnParticleList.cpp
//
//  History:    23-June-99   PKeet  Created
//
//	To do:		
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"
#include "UnParticleList.h"
#include "UnParticle.h"
#include "hp2_rs.h"


/*-----------------------------------------------------------------------------
	Private implementation for UParticleList.
-----------------------------------------------------------------------------*/

class UParticleListPriv: public UParticleList
{
	DECLARE_CLASS(UParticleListPriv,UParticleList,CLASS_Transient,Engine)
	void* RustList;		// Rust particle list (crates/hp2-engine/src/particle.rs); owns the UParticle nodes.
	AActor* ActorOwner;

public:

	// Construction.
	UParticleListPriv(AActor* Owner = NULL)
	{
		ActorOwner = Owner;
		RustList = hp2_rs_engine_particle_list_new();
		WorldBox = FBox(0);
	}

	// UObject interface.
	void Destroy()
	{
		guard(UParticleListPriv::Destroy);

		if (ActorOwner)
		{
			// Detach from owner.
			AParticleFX* ParticleFX = Cast<AParticleFX>(ActorOwner);
			if (ParticleFX)
				ParticleFX->ParticleList = 0;
			ActorOwner = 0;
		}
		
		Super::Destroy();
		hp2_rs_engine_particle_list_free( RustList );
		RustList = NULL;
		unguard;
	}

	// UParticleList interface.
	UParticle* StartParticle()
	{
		return (UParticle*)hp2_rs_engine_particle_list_start( RustList );
	}

	UParticle* NextParticle()
	{
		return (UParticle*)hp2_rs_engine_particle_list_next( RustList );
	}

	void Add(UParticle& Particle)
	{
		hp2_rs_engine_particle_list_add( RustList, &Particle );
	}

	void Remove(UParticle* Particle)
	{
		hp2_rs_engine_particle_list_remove( RustList, Particle );
	}

	void RemoveCurrent()
	{
		hp2_rs_engine_particle_list_remove_current( RustList );
	}

	void RemoveFromFront()
	{
		hp2_rs_engine_particle_list_remove_from_front( RustList );
	}

	void RemoveAll()
	{
		hp2_rs_engine_particle_list_remove_all( RustList );
	}

	int Size()
	{
		return hp2_rs_engine_particle_list_size( RustList );
	}

	UParticle* GetParticle( int Id )
	{
		return (UParticle*)hp2_rs_engine_particle_list_get_particle( RustList, Id );
	}

	void UpdateBox()
	{
		hp2_rs_engine_particle_list_update_box( RustList, &WorldBox );
	}
};

UParticleList* UParticleList::Create(AActor* Owner)
{
	return new(Owner->GetOuter()) UParticleListPriv(Owner);
}

IMPLEMENT_CLASS(UParticleList);
IMPLEMENT_CLASS(UParticleListPriv);
void RegisterParticleListPrivateClass()
{
	UParticleListPriv::StaticClass();
}


/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/
