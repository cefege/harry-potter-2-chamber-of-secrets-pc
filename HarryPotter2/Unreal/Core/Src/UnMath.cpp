/*=============================================================================
	UnMath.cpp: Unreal math routines, implementation of FGlobalMath class
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
		* FQuat (quaternions) class added - Erik de Neve
=============================================================================*/

#include "CorePrivate.h"

/*-----------------------------------------------------------------------------
	FGlobalMath constructor.
-----------------------------------------------------------------------------*/
//
// Set up the tables required for fast square root computation.
//
static void SetupTable( FLOAT* ManTbl, FLOAT* ExpTbl, FLOAT Power )
{
	union {FLOAT F; DWORD D;} Temp;

	Temp.F = 1.0;
	for( DWORD i=0; i<(1<<APPROX_EXP_BITS); i++ )
	{
		Temp.D = (Temp.D & 0x007fffff ) + (i << (32-APPROX_EXP_BITS));
		ExpTbl[ i ] = appPow( Abs(Temp.F), Power );
		if( appIsNan(ExpTbl[ i ]) )
			ExpTbl[ i ]=0.0;
		//debugf("exp [%f] %i = %f",Power,i,ExpTbl[i]);
	}

	Temp.F = 1.0;
	for( DWORD i=0; i<(1<<APPROX_MAN_BITS); i++ )
	{
		Temp.D = (Temp.D & 0xff800000 ) + (i << (32-APPROX_EXP_BITS-APPROX_MAN_BITS));
		ManTbl[ i ] = appPow( Abs(Temp.F), Power );
		if( appIsNan(ManTbl[ i ]) )
			ManTbl[ i ]=0.0;
		//debugf("man [%f] %i = %f",i,Power,ManTbl[i]);
	}
}

// Constructor.
FGlobalMath::FGlobalMath()
:	WorldMin			(-32700.0,-32700.0,-32700.0),
	WorldMax			(32700.0,32700.0,32700.0),
	UnitCoords			(FVector(0,0,0),FVector(1,0,0),FVector(0,1,0),FVector(0,0,1)),
	UnitScale			(FVector(1,1,1),0.0,SHEER_ZX),
	ViewCoords			(FVector(0,0,0),FVector(0,1,0),FVector(0,0,-1),FVector(1,0,0))
{
	// Init base angle table.
	{for( INT i=0; i<NUM_ANGLES; i++ )
		TrigFLOAT[i] = appSin((FLOAT)i * 2.0f * PI / (FLOAT)NUM_ANGLES);}

	// Setup square root tables.
	for( DWORD D=0; D< (1<< APPROX_MAN_BITS ); D++ )
	{
		union {FLOAT F; DWORD D;} Temp;
		Temp.F = 1.0;
		Temp.D = (Temp.D & 0xff800000 ) + (D << (23 - APPROX_MAN_BITS));
		Temp.F = appSqrt(Temp.F);
		Temp.D = (Temp.D - ( 64 << 23 ) );   // exponent bias re-adjust
		SqrtManTbl[ D ] = (FLOAT)(Temp.F * appSqrt(2.0)); // for odd exponents
		SqrtManTbl[ D + (1 << APPROX_MAN_BITS) ] =  (FLOAT) (Temp.F * 2.0f);
	}
	SetupTable(DivSqrtManTbl,DivSqrtExpTbl,-0.5);
	SetupTable(DivManTbl,    DivExpTbl,    -1.0);
}


/*-----------------------------------------------------------------------------
	Conversion functions.
-----------------------------------------------------------------------------*/

// Return the FRotator corresponding to the direction that the vector
// is pointing in.  Sets Yaw and Pitch to the proper numbers, and sets
// roll to zero because the roll can't be determined from a vector.
FRotator FVector::Rotation()
{
	FRotator R;

	// Find yaw.
	R.Yaw = (INT)(appAtan2(Y,X) * (FLOAT)MAXWORD / (2.f*PI));

	// Find pitch.
	R.Pitch = (INT)(appAtan2(Z,appSqrt(X*X+Y*Y)) * (FLOAT)MAXWORD / (2.f*PI));

	// Find roll.
	R.Roll = 0;

	return R;
}

//
// Find good arbitrary axis vectors to represent U and V axes of a plane
// given just the normal.
//
void FVector::FindBestAxisVectors( FVector& Axis1, FVector& Axis2 )
{
	guard(FindBestAxisVectors);

	FLOAT NX = Abs(X);
	FLOAT NY = Abs(Y);
	FLOAT NZ = Abs(Z);

	// Find best basis vectors.
	if( NZ>NX && NZ>NY )	Axis1 = FVector(1,0,0);
	else					Axis1 = FVector(0,0,1);

	Axis1 = (Axis1 - *this * (Axis1 | *this)).SafeNormal();
	Axis2 = Axis1 ^ *this;

	unguard;
}

/*-----------------------------------------------------------------------------
	Matrix inversion.
-----------------------------------------------------------------------------*/

//
// Coordinate system inverse.
//
FCoords FCoords::Inverse() const
{
	FLOAT RDet = 1.f / FTriple( XAxis, YAxis, ZAxis );
	FCoords Inv
	(	FVector(0)
	,	RDet * FVector
		(	(YAxis.Y * ZAxis.Z - YAxis.Z * ZAxis.Y)
		,	(ZAxis.Y * XAxis.Z - ZAxis.Z * XAxis.Y)
		,	(XAxis.Y * YAxis.Z - XAxis.Z * YAxis.Y) )
	,	RDet * FVector
		(	(YAxis.Z * ZAxis.X - ZAxis.Z * YAxis.X)
		,	(ZAxis.Z * XAxis.X - XAxis.Z * ZAxis.X)
		,	(XAxis.Z * YAxis.X - XAxis.X * YAxis.Z))
	,	RDet * FVector
		(	(YAxis.X * ZAxis.Y - YAxis.Y * ZAxis.X)
		,	(ZAxis.X * XAxis.Y - ZAxis.Y * XAxis.X)
		,	(XAxis.X * YAxis.Y - XAxis.Y * YAxis.X) )
	);
	Inv.Origin = -Origin << Inv;
	return Inv;
}

FCoords& FCoords::operator/=( const FCoords& T )
{
	// Regular forward transform.
	XAxis  = XAxis.X * T.XAxis  +  XAxis.Y * T.YAxis  +  XAxis.Z * T.ZAxis;
	YAxis  = YAxis.X * T.XAxis  +  YAxis.Y * T.YAxis  +  YAxis.Z * T.ZAxis;
	ZAxis  = ZAxis.X * T.XAxis  +  ZAxis.Y * T.YAxis  +  ZAxis.Z * T.ZAxis;
	Origin = Origin.X * T.XAxis  +  Origin.Y * T.YAxis  +  Origin.Z * T.ZAxis + T.Origin;
	return *this;
}



//
// Convert this orthogonal coordinate system to a rotation.
//
FRotator FCoords::OrthoRotation() const
{
	FRotator R
	(
		(INT)(appAtan2( XAxis.Z, appSqrt(Square(XAxis.X)+Square(XAxis.Y)) ) * 32768.f / PI),
		(INT)(appAtan2( XAxis.Y, XAxis.X                                  ) * 32768.f / PI),
		0
	);
	FCoords S = GMath.UnitCoords / R;
	R.Roll = (INT)(appAtan2( ZAxis | S.YAxis, YAxis | S.YAxis ) * 32768.f / PI);
	return R;
}

/*-----------------------------------------------------------------------------
	FSphere implementation.
-----------------------------------------------------------------------------*/

//
// Compute a bounding sphere from an array of points.
//
FSphere::FSphere( const FVector* Pts, INT Count )
: FPlane(0,0,0,0)
{
	guard(FSphere::FSphere);
	if( Count )
	{
		FBox Box( Pts, Count );
		*this = FSphere( (Box.Min+Box.Max)/2, 0 );
		for( INT i=0; i<Count; i++ )
		{
			FLOAT Dist = FDistSquared(Pts[i],*this);
			if( Dist > W )
				W = Dist;
		}
		W = appSqrt(W) * 1.001f;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	FBox implementation.
-----------------------------------------------------------------------------*/

FBox::FBox( const FVector* Points, INT Count )
: Min(0,0,0), Max(0,0,0), IsValid(0)
{
	guard(FBox::FBox);
	for( INT i=0; i<Count; i++ )
		*this += Points[i];
	unguard;
}

FBox::FBox( const FCoords& Coords )
{
	Min = Max = Coords.Origin;

	// Expand box by extents in each axis.
	for( int i=0; i<3; i++ )
	{
		const FVector& Ci = (&Coords.XAxis)[i];
		if (Ci.X >= 0.0f)	Max.X += Ci.X;
		else				Min.X += Ci.X;
		if (Ci.Y >= 0.0f)	Max.Y += Ci.Y;
		else				Min.Y += Ci.Y;
		if (Ci.Z >= 0.0f)	Max.Z += Ci.Z;
		else				Min.Z += Ci.Z;
	}
	IsValid = true;
}

FBox::FBox( const FBox& Box, const FCoords& Coords )
{
	*this = FBox( Box << Coords );
}

FCoords FBox::GetCoords() const
{
	return FCoords
	( 
		Min,
		FVector( Max.X-Min.X, 0, 0 ),
		FVector( 0, Max.Y-Min.Y, 0 ),
		FVector( 0, 0, Max.Z-Min.Z )
	);
}

FCoords operator <<( const FBox& Box, const FCoords& Coords )
{
	return FCoords
	(
		Box.Min << Coords,
		Coords.XAxis * (Box.Max.X - Box.Min.X),
		Coords.YAxis * (Box.Max.Y - Box.Min.Y),
		Coords.ZAxis * (Box.Max.Z - Box.Min.Z)
	);
}


// SafeNormal
FVector FVector::SafeNormal() const
{
	FLOAT SquareSum = X*X + Y*Y + Z*Z;
	if( SquareSum < SMALL_NUMBER )
		return FVector( 0.f, 0.f, 0.f );

	FLOAT Size = appSqrt(SquareSum); 
	FLOAT Scale = 1.f/Size;
	return FVector( X*Scale, Y*Scale, Z*Scale );
}


/*-----------------------------------------------------------------------------
	FQuat support functions
-----------------------------------------------------------------------------*/

FQuat SlerpQuat(const FQuat &quat1,const FQuat &quat2, float slerp)
{
	// Get cosine of angle betweel quats.
	float cosom = quat1 | quat2;
	float abs_cosom = Abs(cosom);

	if( abs_cosom < 1.f )
	{
		float omega = appAcos(abs_cosom);
		float sininv = 1.f/appSin(omega);
		float scale0 = appSin((1.f - slerp) * omega) * sininv;
		float scale1 = appSin(slerp * omega) * sininv;
		if( cosom < 0.f )
			scale1 = -scale1;

		FQuat result;
		result.X = scale0 * quat1.X + scale1 * quat2.X;
		result.Y = scale0 * quat1.Y + scale1 * quat2.Y;
		result.Z = scale0 * quat1.Z + scale1 * quat2.Z;
		result.W = scale0 * quat1.W + scale1 * quat2.W;
		result.Normalize();
		return result;
	}
	else
	{
		return quat1;
	}
}

// Convert quaternion to transformation matrix, zeroing Origin.
FCoords::FCoords(const FPlace& InP)
:	Origin(InP.Pos)
{
	FLOAT wx, wy, wz, xx, yy, yz, xy, xz, zz, x2, y2, z2;

	x2 = InP.Quat.X + InP.Quat.X;  y2 = InP.Quat.Y + InP.Quat.Y;  z2 = InP.Quat.Z + InP.Quat.Z;
	xx = InP.Quat.X * x2;   xy = InP.Quat.X * y2;   xz = InP.Quat.X * z2;
	yy = InP.Quat.Y * y2;   yz = InP.Quat.Y * z2;   zz = InP.Quat.Z * z2;
	wx = InP.Quat.W * x2;   wy = InP.Quat.W * y2;   wz = InP.Quat.W * z2;

	XAxis.X = 1.0f - (yy + zz);
	XAxis.Y = xy - wz;
	XAxis.Z = xz + wy;

	YAxis.X = xy + wz;
	YAxis.Y = 1.0f - (xx + zz);
	YAxis.Z = yz - wx;

	ZAxis.X = xz - wy;
	ZAxis.Y = yz + wx;
	ZAxis.Z = 1.0f - (xx + yy);
};




/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
