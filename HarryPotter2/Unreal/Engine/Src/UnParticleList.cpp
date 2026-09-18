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
#include <math.h>
#include <list>

using namespace std;


/*-----------------------------------------------------------------------------
	Private implementation for UParticleList.
-----------------------------------------------------------------------------*/

class UParticleListPriv: public UParticleList
{
	DECLARE_CLASS(UParticleListPriv,UParticleList,CLASS_Transient,Engine)
	typedef list<UParticle> TParticles;

	TParticles Particles;
	TParticles::iterator itParticles;
	bool bAdvance;			// Whether to advance to next particle.
	AActor* ActorOwner;

public:

	// Construction.
	UParticleListPriv(AActor* Owner = NULL)
	{
		ActorOwner = Owner;
		itParticles = Particles.begin();
		bAdvance = true;
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
		unguard;
	}

	// UParticleList interface.
	UParticle* StartParticle()
	{
		// Set the iterator to the first particle on the list.
		itParticles = Particles.begin();

		// Return the first particle.
		return GetCurrentParticle();
	}

	UParticle* NextParticle()
	{
		// Set the iterator to the next particle on the list.
		if (bAdvance)
			++itParticles;
		bAdvance = true;

		// Return the current particle.
		return GetCurrentParticle();
	}

	inline UParticle* GetCurrentParticle()
	{
		// Return a null pointer if there are no particles on the list.
		if (itParticles == Particles.end())
		{
			bAdvance = false;
			return 0;
		}

		// Return the first particle.
		return &(*itParticles);
	}

	void Add(UParticle& Particle)
	{
		Particles.push_back(Particle);
	}

	void Remove(UParticle* Particle)
	{
		// If the particle is the currently selected particle, remove and reset the iterator.
		if (&(*itParticles) == Particle)
		{
			itParticles = Particles.erase(itParticles);
			bAdvance = false;
			return;
		}

		// Find the particle.
		TParticles::iterator it = Particles.begin();
		for (; it != Particles.end(); ++it)
			if (&(*it) == Particle)
				break;

		// If the particle cannot be found, nothing more is required.
		if (it == Particles.end())
			return;

		// Remove the particle.
		Particles.erase(it);
	}

	void RemoveCurrent()
	{
		itParticles = Particles.erase(itParticles);
		bAdvance = false;
	}

	void RemoveFromFront()
	{
		Particles.pop_front();
	}

	void RemoveAll()
	{
		Particles.clear();
	}

	int Size()
	{
		const SIZE_T Count = Particles.size();
		check( Count<=static_cast<SIZE_T>(MAXINT) );
		return static_cast<INT>(Count);
	}

	UParticle* GetParticle( int Id )
	{
		// Find the particle.
		TParticles::iterator it = Particles.begin();
		for (; it != Particles.end(); ++it)
			if ( it->Id == Id )
				return &(*it);
			
		return NULL;	
	}

	void UpdateBox()
	{
		WorldBox = FBox(0);
		for( TParticles::iterator it = Particles.begin(); it != Particles.end(); ++it )
		{
			const UParticle& particle = *it;
			FVector Size( Max(particle.Width, particle.Length)*0.5f );
			WorldBox += particle.Position - Size;
			WorldBox += particle.Position + Size;
		}
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
