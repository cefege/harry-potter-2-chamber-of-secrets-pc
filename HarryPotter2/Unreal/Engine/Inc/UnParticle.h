//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999
//
//  File:       UnParticle.h
//
//  Contents:   An indvidual particle that is part of a particle system.
//
//  Classes:    UParticle
//
//  Functions:  
//
//  History:    23-June-99   PKeet  Created
//
//	To do:		
//
//---------------------------------------------------------------------------

/*-----------------------------------------------------------------------------
	Forward declarations.
-----------------------------------------------------------------------------*/

/*-----------------------------------------------------------------------------
	UParticle
-----------------------------------------------------------------------------*/

class ENGINE_API UParticle
{
public:
	FVector LastPosition;	// Position in the world as of last update
	FVector Position;		// Current Position in the world
		
	FVector Velocity;		// Current Speed and Direction
		
	FLOAT	Age;			// Age in seconds
	FLOAT	Lifetime;		// Age at which to "die"
	
	FLOAT	Alpha;			// Current Alpha value
	FLOAT	AlphaFull;		// Full alpha value after growing.
	FLOAT	AlphaDelta;		// Alpha to add every update ( in units per sec )

	FPlane	Color;			// Current RGB color
	FPlane  ColorDelta;		// RGB Color to add ( in units per sec )
	
	FLOAT	Width;			// Horizontal dimension (local space)	
	FLOAT	WidthFull;		// Full width value after growing.
	FLOAT	WidthDelta;		// in units per sec
	FLOAT	Length;			// or Height
	FLOAT	LengthFull;		// Full length value after growing.
	FLOAT	LengthDelta;	// in units per sec
	FLOAT	Tail;			// For fluid this is the length of the tail end of the particle
	FLOAT   DripTimer;      // Time to reach full scale when dripping
	
	FLOAT	ChaosDelay;		// Time before next Chaotic impulse
	
	FLOAT	Spin;			// current rotation
	FLOAT	SpinRate;		// current rate of rotation

	INT		Id;				// unique identifier to interface with script
	INT		PriorityTag;	// A 'hint' for the renderer about the priority of this particle.  Used for LOD

	// Constructor.
	UParticle
	(
		const FVector& Pos = FVector(0.0f, 0.0f, 0.0f),
		const FVector& Vel = FVector(0.0f, 0.0f, 0.0f),
		const AParticleFX* System = NULL
	);
	
	// Moves the particle, and returns 'true' if it has not been destroyed.
	bool Update( const FVector& SystemGravity, const FLOAT SystemDamping, const FVector& SystemWind, ULevel* Level, FLOAT Timestep = 0.05f, AParticleFX* System = NULL);

	// Returns true if the particle is still within the world.
	bool bInWorld() const;

	// Returns true if the particle collides with geometry.
	bool bCollide(ULevel* Level) const;
};

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/
