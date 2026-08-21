/*=============================================================================
	UnPrim.cpp: Unreal primitive functions.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"

/*----------------------------------------------------------------------------
	UPrimitive object implementation.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UPrimitive);

/*----------------------------------------------------------------------------
	UPrimitive collision checking.
----------------------------------------------------------------------------*/

struct FCheck
{
	FVector			End;
	FVector			Start;
	FVector			Extent;
	AActor*			Owner;

	FCheck
	( 
		FVector			InEnd,
		FVector			InStart,
		FVector			InExtent,
		AActor*			InOwner = NULL
	)
	: End(InEnd), Start(InStart), Extent(InExtent), Owner(InOwner)
	{}

	FCheck
	( 
		FVector			InStart,
		FVector			InExtent,
		AActor*			InOwner = NULL
	)
	: End(InStart), Start(InStart), Extent(InExtent), Owner(InOwner)
	{}
};

// Serialize.
void UPrimitive::Serialize( FArchive& Ar )
{
	guard(UPrimitive::Serialize);
	Super::Serialize( Ar );

	Ar << BoundingBox << BoundingSphere;

	unguard;
}

void UPrimitive::ValidateActor( AActor* Owner )
{
}

FVector UPrimitive::GetCollisionExtent( const AActor* Owner ) const
{
	return FVector( Owner->CollisionRadius, Owner->CollisionRadius, Owner->CollisionHeight );
}

//
// GetCollisionBoundingBox.
// Treats the primitive as a cylinder.
// For base cylinder type, CollisionWidth is a Z offset applied to the center.
//
FBox UPrimitive::GetCollisionBoundingBox( const AActor *Owner, bool bWorld ) const
{
	FVector Extent( Owner->CollisionRadius, Owner->CollisionRadius, Owner->CollisionHeight );
	FVector Loc(0, 0, Owner->CollisionWidth );
	if( bWorld )
		Loc += Owner->Location;
	return FBox( Loc - Extent, Loc + Extent );
}

//
// PointCheck.
// Treats the primitive as a cylinder.
//
UBOOL UPrimitive::PointCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			Location,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	guard(UPrimitive::PointCheck);
	if
	(	Owner
	&&	Square(Owner->Location.Z+Owner->CollisionWidth-Location.Z)                                      < Square(Owner->CollisionHeight+Extent.Z)
	&&	Square(Owner->Location.X-Location.X)+Square(Owner->Location.Y-Location.Y) < Square(Owner->CollisionRadius+Extent.X) )
	{
		// Hit.
		Result.Actor    = Owner;
		Result.Normal   = (Location - Owner->Location).SafeNormal();
		if     ( Result.Normal.Z < -0.5 ) Result.Location = FVector( Location.X, Location.Y, Owner->Location.Z - Extent.Z);
		else if( Result.Normal.Z > +0.5 ) Result.Location = FVector( Location.X, Location.Y, Owner->Location.Z - Extent.Z);
		else                              Result.Location = (Owner->Location - Extent.X * (Result.Normal*FVector(1,1,0)).SafeNormal()) + FVector(0,0,Location.Z);
		return 0;
	}
	else return 1;
	unguard;
}

//
// LineCheck.
// Treats the primitive as a cylinder.
//
UBOOL UPrimitive::LineCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	guard(UPrimitive::LineCheck);

	if( !Owner )
		return 1;

	// Treat this actor as a cylinder.
	FVector NetExtent = Extent + Owner->GetCylinderExtent();

	// Quick X reject.
	FLOAT MaxX = Owner->Location.X + NetExtent.X;
	if( Start.X>=MaxX && End.X>=MaxX )
		return 1;
	FLOAT MinX = Owner->Location.X - NetExtent.X;
	if( Start.X<=MinX && End.X<=MinX )
		return 1;

	// Quick Y reject.
	FLOAT MaxY = Owner->Location.Y + NetExtent.Y;
	if( Start.Y>=MaxY && End.Y>=MaxY )
		return 1;
	FLOAT MinY = Owner->Location.Y - NetExtent.Y;
	if( Start.Y<=MinY && End.Y<=MinY )
		return 1;

	// Quick Z reject.
	FLOAT TopZ = Owner->Location.Z + Owner->CollisionWidth + NetExtent.Z;
	if( Start.Z>=TopZ && End.Z>=TopZ )
		return 1;
	FLOAT BotZ = Owner->Location.Z + Owner->CollisionWidth - NetExtent.Z;
	if( Start.Z<=BotZ && End.Z<=BotZ )
		return 1;

	// Clip to top of cylinder.
	FLOAT T0=0.f, T1=1.f;
	if( Start.Z>=TopZ && End.Z<TopZ )
	{
		T0 = (TopZ - Start.Z)/(End.Z - Start.Z);
		Result.Normal = FVector(0,0,1);
	}
	else if( Start.Z<TopZ && End.Z>TopZ )
		T1 = (TopZ - Start.Z)/(End.Z - Start.Z);

	// Clip to bottom of cylinder.
	if( Start.Z<=BotZ && End.Z>BotZ )
	{
		T0 = (BotZ - Start.Z)/(End.Z - Start.Z);
		Result.Normal = FVector(0,0,-1);
	}
	else if( Start.Z>BotZ && End.Z<BotZ )
		T1 = (BotZ - Start.Z)/(End.Z - Start.Z);

	// Test setup.
	FLOAT   Kx        = Start.X - Owner->Location.X;
	FLOAT   Ky        = Start.Y - Owner->Location.Y;

	// 2D circle clip about origin.
	FLOAT   Vx        = End.X - Start.X;
	FLOAT   Vy        = End.Y - Start.Y;
	FLOAT   A         = Vx*Vx + Vy*Vy;
	FLOAT   B         = 2.f * (Kx*Vx + Ky*Vy);
	FLOAT   C         = Kx*Kx + Ky*Ky - Square(NetExtent.X);
	FLOAT   Discrim   = B*B - 4.f*A*C;

	// If already inside sphere, oppose further movement inward.
	if( C<Square(1.f) && Start.Z>BotZ && Start.Z<TopZ )
	{
		FLOAT Dir = ((End-Start)*FVector(1,1,0)) | (Start-Owner->Location);
		if( Dir < -0.1f )
		{
			Result.Time      = 0.f;
			Result.Location  = Start;
			Result.Normal    = ((Start-Owner->Location)*FVector(1,1,0)).SafeNormal();
			Result.Actor     = Owner;
			Result.Primitive = NULL;
			return 0;
		}
		else return 1;
	}

	// No intersection if discriminant is negative.
	if( Discrim < 0.0f )
		return 1;

	// Unstable intersection if velocity is tiny.
	if( A < Square(0.0001f) )
	{
		// Outside or on circle.
		if( C >= 0 )
			return 1;
	}
	else
	{
		// Compute intersection times.
		Discrim   = appSqrt(Discrim);
		FLOAT R2A = 0.5f/A;
		T1        = ::Min( T1, +(Discrim-B) * R2A );
		FLOAT T   = -(Discrim+B) * R2A;
		if( T >= T0 )
		{
			T0 = T;
			Result.Normal   = (Start + (End-Start)*T0 - Owner->Location);
			Result.Normal.Z = 0;
			Result.Normal.Normalize();
		}
		if( T0 >= T1 )	// If touched in one point (tangent) or passed cyclinder end caps
			return 1;
	}
	Result.Time      = Clamp( T0-0.001f, 0.f, 1.f );
	Result.Location  = Start + (End-Start) * Result.Time;
	Result.Actor     = Owner;
	Result.Primitive = NULL;
	return 0;

	unguard;
}

void UPrimitive::DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags )
{
	// Draw an aligned cylinder.
	Render->DrawCylinder( Frame, Color, LineFlags, FCoords(Owner->Location + FVector(0,0,Owner->CollisionWidth)), Owner->CollisionRadius, Owner->CollisionHeight );
}

/*----------------------------------------------------------------------------
	UOrientedCylinder.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UOrientedCylinder);

FBox UOrientedCylinder::GetCollisionBoundingBox( const AActor* Owner, bool bWorld) const 
{
	FVector Ext( Owner->CollisionRadius, Owner->CollisionRadius, Owner->CollisionHeight );
	FBox Box(Ext);
	if( bWorld )
		return Box.TransformBy( Owner->ToWorld() );
	else
		return Box;
}

void UOrientedCylinder::DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags )
{
	Render->DrawCylinder( Frame, Color, LineFlags, Owner->ToLocal(), Owner->CollisionRadius, Owner->CollisionHeight );
}

FVector TransformExtent( const FVector& V, const FCoords& C )
{
	// Very inefficient, but uses existing routines.
	FBox Box(V);
	Box = Box.TransformBy(C);
	return (Box.Max - Box.Min)*0.5f;
}

// Collision checks. Re-orient the testers, and call parent.
UBOOL UOrientedCylinder::PointCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			Location,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	// Convert inputs to local rotation.
	FCoords Local = Owner->ToLocal();
	Location = Location.TransformPointBy(Local) + Owner->Location;
	Extent = TransformExtent(Extent, Local);
	if( UCylinder::PointCheck( Result, Owner, Location, Extent, ExtraNodeFlags ) )
		return 1;

	// Convert result the other way.
	Local = Local.Transpose();
	Result.Location = (Result.Location - Owner->Location).TransformPointBy(Local);
	Result.Normal = Result.Normal.TransformVectorBy(Local);
	return 0;
}

UBOOL UOrientedCylinder::LineCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	FCoords Local = Owner->ToLocal();
	Start = Start.TransformPointBy(Local) + Owner->Location;
	End = End.TransformPointBy(Local) + Owner->Location;
	Extent = TransformExtent(Extent, Local);
	if( UCylinder::LineCheck( Result, Owner, End, Start, Extent, ExtraNodeFlags ) )
		return 1;

	// Convert result the other way.
	Local = Local.Transpose();
	Result.Location = (Result.Location - Owner->Location).TransformPointBy(Local);
	Result.Normal = Result.Normal.TransformVectorBy(Local);
	return 0;
}

/*----------------------------------------------------------------------------
	UAlignedOvalCylinder.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UAlignedOvalCylinder);

//
// GetCollisionBoundingBox.
// Treats the primitive as an axis-aligned oval cylinder (vertically extruded ellipse).
// CollisionWidth and CollisionRadius are the radial lengths of the X & Y axes of the
// ellipse, respectively.
//
FBox UAlignedOvalCylinder::GetCollisionBoundingBox( const AActor *Owner, bool bWorld ) const
{
	FVector Extent( Owner->CollisionWidth, Owner->CollisionRadius, Owner->CollisionHeight );
	FVector Loc( 0, 0, 0 );
	if( bWorld )
		Loc += Owner->Location;
	return FBox( Loc - Extent, Loc + Extent );
}

//
// PointCheck.
// Treats the primitive as an axis-aligned oval cylinder (vertically extruded ellipse).
//
UBOOL UAlignedOvalCylinder::PointCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			Location,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	guard(UAlignedOvalCylinder::PointCheck);

	FLOAT		YXAxisRatio;
	FLOAT		fOriginalOwnerLocationX;
	FLOAT		fOriginalOwnerCollisionWidth;
	UBOOL		bOutside;

	if ( !Owner || Owner->CollisionRadius < 0.0001 || Owner->CollisionWidth < 0.0001 )
		return 1;

	// Scale all X components into Y scale so cylinder can be treated as circular
	YXAxisRatio = Owner->CollisionRadius / Owner->CollisionWidth;
	fOriginalOwnerLocationX = Owner->Location.X;
	fOriginalOwnerCollisionWidth = Owner->CollisionWidth;
	Owner->CollisionWidth = 0;		// Regular cylinder checks will mistake this as height offset, so zero it;

	Owner->Location.X	*= YXAxisRatio;
	Location.X			*= YXAxisRatio;
	Extent.X			*= YXAxisRatio;

	// Perform normal circular cylinder point check
	bOutside = UCylinder::PointCheck( Result, Owner, Location, Extent, ExtraNodeFlags );
	Owner->Location.X = fOriginalOwnerLocationX;
	Owner->CollisionWidth = fOriginalOwnerCollisionWidth;
	if ( bOutside )
		return 1;

	// Undo scale on result
	Result.Location.X	/= YXAxisRatio;
	Result.Normal.X		/= YXAxisRatio;
	Result.Normal.Normalize();
	return 0;

	unguard;
}

//
// LineCheck.
// Treats the primitive as an axis-aligned oval cylinder (vertically extruded ellipse).
//
UBOOL UAlignedOvalCylinder::LineCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	guard(UAlignedOvalCylinder::LineCheck);

	FLOAT		YXAxisRatio;
	FLOAT		fOriginalOwnerLocationX;
	FLOAT		fOriginalOwnerCollisionWidth;
	UBOOL		bOutside;

	if ( !Owner || Owner->CollisionRadius < 0.0001 || Owner->CollisionWidth < 0.0001 )
		return 1;

	// Scale all X components into Y scale so cylinder can be treated as circular
	YXAxisRatio = Owner->CollisionRadius / Owner->CollisionWidth;
	fOriginalOwnerLocationX = Owner->Location.X;
	fOriginalOwnerCollisionWidth = Owner->CollisionWidth;
	Owner->CollisionWidth = 0;		// Regular cylinder checks will mistake this as height offset, so zero it;

	Owner->Location.X	*= YXAxisRatio;
	End.X				*= YXAxisRatio;
	Start.X				*= YXAxisRatio;
	Extent.X			*= YXAxisRatio;

	// Perform normal circular cylinder line check
	bOutside = UCylinder::LineCheck( Result, Owner, End, Start, Extent, ExtraNodeFlags );
	Owner->Location.X = fOriginalOwnerLocationX;
	Owner->CollisionWidth = fOriginalOwnerCollisionWidth;
	if ( bOutside )
		return 1;

	// Undo scale on result
	Result.Location.X	/= YXAxisRatio;
	Result.Normal.X		/= YXAxisRatio;
	Result.Normal.Normalize();
	return 0;

	unguard;
}

// Treats the primitive as an axis-aligned oval cylinder (vertically extruded ellipse).
void UAlignedOvalCylinder::DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags )
{
	// Draw an aligned oval cylinder.
	Render->DrawCylinder(
		Frame,
		Color,
		LineFlags,
		FCoords(Owner->Location),
		Owner->CollisionRadius,
		Owner->CollisionHeight,
		Owner->CollisionWidth
	);
}

/*----------------------------------------------------------------------------
	UOrientedOvalCylinder.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UOrientedOvalCylinder);

FBox UOrientedOvalCylinder::GetCollisionBoundingBox( const AActor* Owner, bool bWorld) const 
{
	FVector Ext( Owner->CollisionWidth, Owner->CollisionRadius, Owner->CollisionHeight );
	FBox Box(Ext);
	if( bWorld )
		return Box.TransformBy( Owner->ToWorld() );
	else
		return Box;
}

// Collision checks. Re-orient the testers, and call parent.
UBOOL UOrientedOvalCylinder::PointCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			Location,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	// Convert inputs to local rotation.
	FCoords Local = Owner->ToLocal();
	Location = Location.TransformPointBy(Local) + Owner->Location;
	Extent = TransformExtent(Extent, Local);
	if( UAlignedOvalCylinder::PointCheck( Result, Owner, Location, Extent, ExtraNodeFlags ) )
		return 1;

	// Convert result the other way.
	Local = Local.Transpose();
	Result.Location = (Result.Location - Owner->Location).TransformPointBy(Local);
	Result.Normal = Result.Normal.TransformVectorBy(Local);
	return 0;
}

UBOOL UOrientedOvalCylinder::LineCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	FCoords Local = Owner->ToLocal();
	Start = Start.TransformPointBy(Local) + Owner->Location;
	End = End.TransformPointBy(Local) + Owner->Location;
	Extent = TransformExtent(Extent, Local);
	if( UAlignedOvalCylinder::LineCheck( Result, Owner, End, Start, Extent, ExtraNodeFlags ) )
		return 1;

	// Convert result the other way.
	Local = Local.Transpose();
	Result.Location = (Result.Location - Owner->Location).TransformPointBy(Local);
	Result.Normal = Result.Normal.TransformVectorBy(Local);
	return 0;
}

void UOrientedOvalCylinder::DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags )
{
	// Draw an oriented oval cylinder.
	Render->DrawCylinder(
		Frame,
		Color,
		LineFlags,
		Owner->ToLocal(),
		Owner->CollisionRadius,
		Owner->CollisionHeight,
		Owner->CollisionWidth
	);
}

/*----------------------------------------------------------------------------
	FBox collision routines.
----------------------------------------------------------------------------*/

inline bool bIntersects( const FBox& A, const FBox& B, float fTolerance = 0.0 )
{
	// Returns true if boxes overlap -- not just touch.
	// The fTolerance parameter is how much the boxes can overlap and still
	// be considered "non-overlapping"; it's essentially a measure of how
	// "thick" the surface of box A is.
	return   A.Min.X + fTolerance < B.Max.X   &&   A.Max.X - fTolerance > B.Min.X
		&&   A.Min.Y + fTolerance < B.Max.Y   &&   A.Max.Y - fTolerance > B.Min.Y
		&&   A.Min.Z + fTolerance < B.Max.Z   &&   A.Max.Z - fTolerance > B.Min.Z;
}


// Check point containment in a box.
UBOOL PointCheck
(
	FCheckResult&	Result,
 	const FCheck&	Check,
	const FBox&		Box
)
{
	if( Check.Owner )
	{
		// Convert inputs to local space.
		FCoords Local = Check.Owner->ToLocal();

		// Check whether they intersect.
		if( PointCheck
		( 
			Result, 
			FCheck( Check.Start.TransformPointBy(Local), TransformExtent(Check.Extent, Local) ), 
			Box 
		) )
			return 1;

		// Convert results to global space.
		Local = Local.Transpose();
		Result.Normal = Result.Normal.TransformVectorBy(Local);
		Result.Location = Result.Location.TransformPointBy(Local);
		Result.Actor = Check.Owner;
	}
	else
	{
		// Check whether they intersect.
		FBox Source(Check.Start - Check.Extent, Check.Start + Check.Extent);
		if( !bIntersects( Box, Source, 0.003f ) )	// Tolerance compensates for error accumulated by transformations in ::LineCheck
			return 1;

		// Compute where intersection occured on box (point nearest start point)
		Result.Location.X = Clamp( Check.Start.X, Box.Min.X, Box.Max.X );
		Result.Location.Y = Clamp( Check.Start.Y, Box.Min.Y, Box.Max.Y );
		Result.Location.Z = Clamp( Check.Start.Z, Box.Min.Z, Box.Max.Z );

		// Find Box plane that Source intersects least.
		float T = Box.Max.Z - Source.Min.Z;
		Result.Time = T;
		Result.Normal = FVector(0,0,1);
		T = Source.Max.Z - Box.Min.Z;
		if( T < Result.Time )
		{
			Result.Time = T;
			Result.Normal = FVector(0,0,-1);
		}

		T = Box.Max.Y - Source.Min.Y;
		if( T < Result.Time )
		{
			Result.Time = T;
			Result.Normal = FVector(0,1,0);
		}
		T = Source.Max.Y - Box.Min.Y;
		if( T < Result.Time )
		{
			Result.Time = T;
			Result.Normal = FVector(0,-1,0);
		}

		T = Box.Max.X - Source.Min.X;
		if( T < Result.Time )
		{
			Result.Time = T;
			Result.Normal = FVector(1,0,0);
		}
		T = Source.Max.X - Box.Min.X;
		if( T < Result.Time )
		{
			Result.Time = T;
			Result.Normal = FVector(-1,0,0);
		}
	}

	return 0;
}

inline bool Contains( const FBox& Box, const FVector& V, float fTolerance = 0.0 )
{
	// Returns true if point is within the hull of the box, but not on its surface.
	// The fTolerance parameter is how far inside the box the point can be from
	// the surface and still be considered "not inside"; it's essentially a measure
	// of how "thick" the surface is.
	return   V.X > Box.Min.X + fTolerance   &&   V.X < Box.Max.X - fTolerance
		&&   V.Y > Box.Min.Y + fTolerance   &&   V.Y < Box.Max.Y - fTolerance
		&&   V.Z > Box.Min.Z + fTolerance   &&   V.Z < Box.Max.Z - fTolerance;
}

inline void ClipPlane
( 
	FCheckResult&	Result,
	const FCheck&	Check,
	const FBox&		Box,
	const FVector&	Normal,
	FLOAT			D,
	FLOAT			E
)
{
	// Find intersection point.
	if( D > -0.003f && ::Max(0.0f, D) < E*Result.Time )	// Tolerance guarantees that clip check is made
	{													// even if start is slightly inside surface;
		FLOAT T = D/E;									// otherwise, object can creep into the interior.
		if ( T < 0.0f )
			T = 0.0f;
		FVector Mid = Check.Start*(1.f-T) + Check.End*T;
		Mid -= 0.001f * Normal;	// Push in to guarentee inside of this plane
		if( Contains(Box, Mid) )
		{
			T -= 0.001f;		// Push out to put intersection point just outside box
			if ( T < 0.0f )
				T = 0.0f;

			Result.Time = T;
			Result.Location = Check.Start*(1.f-T) + Check.End*T;

			// Move inward by extent.
			Result.Location -= Normal * (Normal|Check.Extent);
			Result.Normal = Normal;
		}
	}
}

UBOOL LineCheck
(
	FCheckResult&	Result,
	const FCheck&	Check,
	const FBox&		Box
)
{
	if( Check.Owner )
	{
		// Convert source vectors to local.
		FCoords Local = Check.Owner->ToLocal();
		if( ::LineCheck
		( 
			Result, 
			FCheck( Check.End.TransformPointBy(Local), Check.Start.TransformPointBy(Local), TransformExtent(Check.Extent, Local) ),
			Box
		) )
			return 1;

		// Convert result the other way.
		Local = Local.Transpose();
		Result.Location = Result.Location.TransformPointBy(Local);
		Result.Normal = Result.Normal.TransformVectorBy(Local);
		Result.Actor = Check.Owner;
		return 0;
	}
	else
	{
		// Check intersection at start.
		if( PointCheck(Result, Check, Box) == 0 )
		{
			// Start is already inside collision box; check intersection at end
			FCheck			ReverseCheck( Check.Start, Check.End, Check.Extent, Check.Owner);
			FCheckResult	ReverseResult;
			if ( PointCheck( ReverseResult, ReverseCheck, Box ) == 0 
			&& ReverseResult.Time > Result.Time )
			{
				// End is moving further into box; it's a hit.
				Result.Time = 0.f;
				return 0;
			}
			else
			{
				// Pretend there's no hit to permit traveling outward
				return 1;
			}
		}

		Result.Time = 1.f;

		FBox BigBox(Box.Min - Check.Extent, Box.Max + Check.Extent);
		ClipPlane( Result, Check, BigBox, FVector(-1,0,0), BigBox.Min.X - Check.Start.X, Check.End.X - Check.Start.X );
		ClipPlane( Result, Check, BigBox, FVector(+1,0,0), Check.Start.X - BigBox.Max.X, Check.Start.X - Check.End.X );
		ClipPlane( Result, Check, BigBox, FVector(0,-1,0), BigBox.Min.Y - Check.Start.Y, Check.End.Y - Check.Start.Y );
		ClipPlane( Result, Check, BigBox, FVector(0,+1,0), Check.Start.Y - BigBox.Max.Y, Check.Start.Y - Check.End.Y );
		ClipPlane( Result, Check, BigBox, FVector(0,0,-1), BigBox.Min.Z - Check.Start.Z, Check.End.Z - Check.Start.Z );
		ClipPlane( Result, Check, BigBox, FVector(0,0,+1), Check.Start.Z - BigBox.Max.Z, Check.Start.Z - Check.End.Z );

		return Result.Time == 1.f;
	}
}

/*----------------------------------------------------------------------------
	UBox.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UBox);

FBox UBox::GetCollisionBoundingBox(const AActor* Owner, bool bWorld) const 
{
	FBox Box( FVector
	(
		Owner->CollisionRadius, 
		Owner->CollisionWidth ? Owner->CollisionWidth : Owner->CollisionRadius,
		Owner->CollisionHeight
	) );
	if( bWorld )
		return Box.TransformBy( Owner->ToWorld() );
	else
		return Box;
}

FVector UBox::GetCollisionExtent(const AActor* Owner) const
{
	// Be lenient in case CollisionWidth unset.
	return FVector
	(
		Owner->CollisionRadius,
		Owner->CollisionWidth ? Owner->CollisionWidth : Owner->CollisionRadius, 
		Owner->CollisionHeight
	);
}

void UBox::DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags )
{
	Render->DrawBox( Frame, Color, LineFlags, GetCollisionBoundingBox(Owner, false), Owner->ToLocal() );
}

UBOOL UBox::PointCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			Location,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	return ::PointCheck( Result, FCheck(Location, Extent, Owner), GetCollisionBoundingBox(Owner, false) );
}

UBOOL UBox::LineCheck
(
	FCheckResult&	Result,
	AActor*			Owner,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	return ::LineCheck( Result, FCheck(End, Start, Extent, Owner), GetCollisionBoundingBox(Owner, false) );
}

/*----------------------------------------------------------------------------
	UBoxPrim.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UBoxPrim);

FBox UBoxPrim::GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const
{
	if( bWorld )
		return BoundingBox.TransformBy( Owner->ToWorld() );
	else
		return BoundingBox;
}

FVector UBoxPrim::GetCollisionExtent( const AActor* Owner ) const
{
	return FVector( Max( -BoundingBox.Min.X, BoundingBox.Max.X ),
					Max( -BoundingBox.Min.Y, BoundingBox.Max.Y ),
					Max( -BoundingBox.Min.Z, BoundingBox.Max.Z ) );
}

void UBoxPrim::ValidateActor( AActor* Owner )
{
	FVector Ext = GetCollisionExtent(Owner);
	Owner->CollisionRadius = Owner->CollisionWidth = Max(Ext.X, Ext.Y);
	Owner->CollisionHeight = Ext.Z;
}

/*----------------------------------------------------------------------------
	FCheckResult surface identification.
----------------------------------------------------------------------------*/

FBspNode* FCheckResult::BspNode( FLOAT Radius ) const
{
	guard(FCheckResult::BspNode);

	// Make sure a model node was hit.
	if( Item == INDEX_NONE )
		return NULL;
	UModel* Model = Cast<UModel>(Primitive);
	if( !Model )
		return NULL;

	FVector Loc = Location;
	FVector Norm = Normal;

	if( Actor && Actor->IsA(AMover::StaticClass()) )
	{
		// Transform results into mover space.
		FCoords C = Actor->ToLocal();
		Loc = Location.TransformPointBy(C);
		Norm = Normal.TransformVectorBy(C);
	}

	INT iNode = Item;
	FBspNode* AnyNode = NULL;
	while (iNode != INDEX_NONE )
	{
		FBspNode* Node = &Model->Nodes(iNode);
		if ( Node->NumVertices > 0 )
		{
			// Make sure it's the same plane (to avoid anomalous trace results).
			if( (Node->Plane | Norm) >= 0.99f )
			{
				AnyNode = Node;

				// Check if this intersection point lies inside this node.
				FVert* Verts = &Model->Verts( Node->iVertPool );
				FVector* PrevVertex = &Model->Points(Verts[Node->NumVertices - 1].pVertex );

				FLOAT Sense = 1.0f;
				INT i;
				for( i=0;i<Node->NumVertices;i++ )
				{
					FVector* Vertex = &Model->Points(Verts[i].pVertex);
					FVector ClipNorm = Node->Plane ^ (*Vertex - *PrevVertex);
					ClipNorm.Normalize();
					FPlane ClipPlane( *Vertex, ClipNorm );

					FLOAT Dot = ClipPlane.PlaneDot( Loc );

					if( Dot*Sense < -Radius )
					{
						if( i == 0 )
							Sense = -1.f;
						else
							break;
					}
					PrevVertex = Vertex;
				}
				if( i == Node->NumVertices )
					return Node;
			}
		}

		// Check next co-planars to see if it contains this intersection point.
		check( Node->iPlane != iNode );
		iNode = Node->iPlane;
	}

	return Radius != 0? NULL : AnyNode;
	unguard;
}

FBspSurf* FCheckResult::BspSurf( FLOAT Radius ) const
{
	FBspNode* Node = BspNode(Radius);
	if( Node )
	{
		UModel* Model = Cast<UModel>(Primitive);
		if( Model )
			return &Model->Surfs(Node->iSurf);
	}
	return NULL;
}

UTexture* FCheckResult::Texture( class UClass* SomeBase ) const
{
	// Find a valid actor.
	if (!Actor || !Actor->XLevel)
		return NULL;

	// If no base class is specified, assume the level base class.
	if (!SomeBase)
	{
		SomeBase = ALevelInfo::StaticClass();
	}
	if (!Actor->IsA(SomeBase))
		return NULL;

	FBspSurf* Surf = BspSurf();
	return Surf ? Surf->Texture : NULL;
}

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/
