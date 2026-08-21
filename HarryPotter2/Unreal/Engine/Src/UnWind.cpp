//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999.
//
//  File:       UnWind.cpp
//
//  Contents:   Implementation of AWind.h
//
//  History:    00-Jan-18	SPeter created
//
//	To do:		Make fluctuation both spatially and temporally variant, but
//				locally coherent.
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"

/*-----------------------------------------------------------------------------
	AWind implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(AWind);

static TArray<AWind*> GWindActors;
static float Clug = 1.f;

static float NoiseGrain = 32.f;
static float SpatialFluc = 1.f;

const int NoiseRepeat = 256;


//+--------------------------------------------------------------------------
template<class T> inline 
void Swap(T& a, T& b)
//
// Swaps the two arguments.
//
{
	T temp = a;
	a = b;
	b = temp;
}



class FIntRandom
{
	unsigned char	IntRand[256];
	bool	bInit;

public:
	FIntRandom()
		: bInit(false)
	{
	}

	void Init()
	{
		if( bInit )
			return;
		bInit = true;

		// Fill table with unique values.
		for_count( i, 256 )
			IntRand[i] = i;

		// Shuffle them.
		for_count( j, 256 )
		{
			Swap( IntRand[j], IntRand[ appRand()%256 ] );
		}
	}

	inline unsigned char operator ()( int i ) const
	{
		checkSlow(bInit);
		return IntRand[i%256];
	}

} GIntRandom;


static inline FVector VecRand( int iX, int iY, int iZ )
{
	FVector Vec;

	int Hash = GIntRandom( iX + GIntRandom( iY + GIntRandom( iZ ) ) );
								Vec.X = Hash/255.f - 0.5f;
	Hash = GIntRandom(Hash);	Vec.Y = Hash/255.f - 0.5f;
	Hash = GIntRandom(Hash);	Vec.Z = Hash/255.f - 0.5f;

	return Vec;
}

static FVector VecRand( const FVector& Loc )
{
	// Add spatial variation: a 3D noise field.
	GIntRandom.Init();

	FVector L = Loc / NoiseGrain;
	int iX = appFloor(L.X),  iY = appFloor(L.Y),  iZ = appFloor(L.Z);
	FVector R( L.X - iX, L.Y - iY, L.Z - iZ ),
			S = FVector(1,1,1) - R;

	// Find random vectors at the 8 corners, and interpolate.
	return VecRand( iX,   iY,   iZ   ) * (S.X * S.Y * S.Z)
		+  VecRand( iX,   iY,   iZ+1 ) * (S.X * S.Y * R.Z)
		+  VecRand( iX,   iY+1, iZ   ) * (S.X * R.Y * S.Z)
		+  VecRand( iX,   iY+1, iZ+1 ) * (S.X * R.Y * R.Z)
		+  VecRand( iX+1, iY,   iZ   ) * (R.X * S.Y * S.Z)
		+  VecRand( iX+1, iY,   iZ+1 ) * (R.X * S.Y * R.Z)
		+  VecRand( iX+1, iY+1, iZ   ) * (R.X * R.Y * S.Z)
		+  VecRand( iX+1, iY+1, iZ+1 ) * (R.X * R.Y * R.Z);
}

AWind::AWind()
{
	// Add to GWindActors.
	GWindActors.AddItem( this );
}

void AWind::Destroy()
{
	// Remove from GWindActors.
	GWindActors.RemoveItem( this );
	Super::Destroy();
}

UBOOL AWind::Tick( FLOAT DeltaTime, enum ELevelTick TickType )
{
	Super::Tick( DeltaTime, TickType );
	if( WindFluctuation )
	{
		//
		// To simulate wind gusts, we indirectly perturb the wind. 
		//

		// Exponentially average FlucVel over the period.
		float Frac = DeltaTime / FlucPeriod();
		float Decay = appExp(-Frac * Clug);

		// Decay fluctuation velocity slowly, then randomly perturb it.
		FlucVel = FlucVel * Decay + VRand() * (FlucFraction() * (1.f - Decay));

		// Decay the actual fluctuation, and apply velocity.
		Fluc *= appExp(-Frac);
		Fluc += FlucVel * DeltaTime;
	}
	return 1;
}

FVector AWind::GetWind( const FVector& Loc )
{
	// Check range.
	float DistSq = (Loc - Location).SizeSquared();
	if( WindSpeed == 0.f || DistSq > Square( Radius() ))
		return FVector(0);

	// Check line of sight.
	if (!bPermeating && !GetLevel()->Model->FastLineCheck(Location, Loc))
		return FVector(0);

	float Dist = appSqrt(DistSq);

	FVector Vec;

	switch( WindSource )
	{
	case LD_Point:
		if( Dist > 0.0f )
			Vec = (Loc - Location) / Dist;
		else
	case LD_Plane:
			Vec = Rotation.Vector();
		break;
	default:
		Vec = FVector(0);
	}

	// Add time fluctuation.
	Vec += Fluc;

	if( WindFluctuation * SpatialFluc != 0 )
		Vec += VecRand(Loc) * (Vec.Size() * FlucFraction() * SpatialFluc);

	// Scale by strength and distance.
	Vec *= WindSpeed * Min( (1.0f - Dist / Radius()) / (1.0f - InnerRadiusFrac()), 1.0f );

	return Vec;
}

FVector AWind::GetTotalWind( ULevel* Level, const FVector& Loc )
{
	FVector TotalWind(0);

	// First, slow implementation. Iterate through all level actors, looking for wind.
	for( INT i=0; i<GWindActors.Num(); i++ )
		TotalWind += GWindActors(i)->GetWind( Loc );

	return TotalWind;
}

// Script interfaces.

void AWind::execGetWind( FFrame& Stack, RESULT_DECL )
{
	P_GET_VECTOR (Loc);
	P_FINISH;

	*(FVector*)Result = GetWind( Loc );
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

