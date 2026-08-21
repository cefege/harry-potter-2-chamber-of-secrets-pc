/*=============================================================================
	UnParticleFX.h.
	Copyright 1999,2000 DreamWorks Interactive. All Rights Reserved.
=============================================================================*/

#define	ONESECOND_UPDATE 0	
#define OFFSCREEN_TICK 0

const float DEGREES_TO_RADIANS=0.01745329f;
const float	RADIANS_TO_DEGREES=57.2957795f;

inline FLOAT DegToRad(FLOAT x){ return x * DEGREES_TO_RADIANS; }
inline FLOAT RadToDeg(FLOAT x){ return x * RADIANS_TO_DEGREES; }

inline FLOAT UseRand(FLOAT x){ if (x > 0.0f) return appFrand()*x; else return 0.0f; }

const int	PriorityTable[10]={9,3,1,7,5,0,6,2,4,8};

struct FFloatParams
{
	float	Base;
	float   Rand;
};

struct FColorParams
{
	FColor	Base;
	FColor  Rand;
};

struct FParticleParams
{
	FVector Position;		// Current Position in the world
	FVector Velocity;		// Current Speed and Direction
		
	float	Lifetime;		// Age at which to "die"
	
	float	Alpha;			// Current Alpha value

	FColor	Color;			// Current RGB color
	
	float	Width;			// Horizontal dimension (local space)	
	float	Length;			// or Height

	float   DripTimer;      // Time to reach full scale when dripping
	
	float	SpinRate;		// current rotation rate
};

struct FParams
{
	float		ParticlesPerSec;	
	float		SourceWidth;
	float		SourceHeight;
	float		SourceDepth;
	float		Period;
	float		Decay;
	float		AngularSpreadWidth;	
	float		AngularSpreadHeight;
	float		Speed;
	float		Lifetime;
	FColor		ColorStart;
	FColor		ColorEnd;
	float		AlphaStart;
	float		AlphaEnd;
	float		SizeWidth;
	float		SizeLength;
	float		SizeEndScale;
	float		SpinRate;
	float		DripTime;
};

 
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
