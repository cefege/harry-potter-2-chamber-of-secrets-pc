//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999
//
//  File:       UnParticleList.h
//
//  Contents:   A list of particles attached to an actor.
//
//  Classes:    UParticleList
//
//  Functions:  
//
//  History:    23-June-99   PKeet  Created
//
//	To do:		Implement a true particle system based on this system.
//
//---------------------------------------------------------------------------

/*-----------------------------------------------------------------------------
	Forward declarations.
-----------------------------------------------------------------------------*/

class UParticle;

/*-----------------------------------------------------------------------------
	UParticleList
-----------------------------------------------------------------------------*/

class ENGINE_API UParticleList : public UPrimitive
{
public:

	DECLARE_ABSTRACT_CLASS(UParticleList,UPrimitive,CLASS_Transient,Engine)

	// Named constructor.
	static UParticleList* Create(AActor* Owner);

	//
	// Iterator interface for particles. Note that the custom interface is designed
	// to allow objects to use the UParticleList interface without knowing about UParticle
	// objects. Functions return a null pointer when the end of the list is reached.
	//

	// Resets iteration to start again from the first particle.
	virtual UParticle* StartParticle()=0;
	
	// Returns the next particle in the list.
	virtual UParticle* NextParticle()=0;

	// Adds a new particle to the list.
	virtual void Add(UParticle& Particle)=0;

	// Removes a particle from the list. Safe to use in the Start...Next loop.
	virtual void Remove(UParticle* Particle)=0;

	// Removes the currently iterating particle, leaving the iteration valid.
	virtual void RemoveCurrent()=0;

	// Clears the list to zero.
	virtual void RemoveAll()=0;

	// Returns the number of elements in the list
	virtual int Size()=0;
	
	// Removed entries from front of list
	virtual void RemoveFromFront()=0;

	// Return a specific particle (usually scripted).
	virtual UParticle* GetParticle( int Id )=0;

	virtual void UpdateBox()=0;

	// Private ParticleFX variables, stored here for privacy.
	FBox WorldBox;
};

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/
