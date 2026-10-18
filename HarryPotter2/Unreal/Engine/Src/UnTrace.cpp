/*=============================================================================
    UnPhys.cpp: Simple physics and occlusion testing for editor
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "HP2PrimTrace.h"

/*---------------------------------------------------------------------------------------
   Primitive bounding boxes.
---------------------------------------------------------------------------------------*/

FBox UModel::GetRenderBoundingBox( const AActor* Owner, UBOOL Exact )
{
	guard(UModel::GetRenderBoundingBox);
	Hp2TraceModelArg M;
	Hp2TraceModelFor( this, M );
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_trace_get_render_bounding_box( &M, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), Exact, Polys != NULL,
		Polys ? Hp2ArrayPtr(Polys->Element) : NULL, Polys ? Polys->Element.Num() : 0, &Box );
	return Box;
	unguard;
}

/*---------------------------------------------------------------------------------------
   Point checks.
---------------------------------------------------------------------------------------*/

//
// See if a box with the specified collision info fits at Point.
//
// If it doesn't fit, returns 0 and sets SuggestedOutVector to a suggested
// vector which might move the point out of the surface it's colliding with.
// SuggestedOutVector is a good guess but it's not guaranteed to be correct.
//
// If it fits, returns 1.
//
UBOOL UModel::PointCheck
(
	FCheckResult&	Hit,
	AActor*			Owner,
	FVector			Location,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	guard(UModel::PointCheck);
	Hp2TraceModelArg M;
	Hp2TraceModelFor( this, M );
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_trace_model_point_check( &Hit, &M, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), &Location, &Extent, ExtraNodeFlags );
	unguard;
}

/*---------------------------------------------------------------------------------------
   Fast line check.
---------------------------------------------------------------------------------------*/

// Fast line check.
BYTE UModel::FastLineCheck( FVector End, FVector Start )
{
	guard(UModel::FastLineCheck);
	Hp2TraceModelArg M;
	Hp2TraceModelFor( this, M );
	return Nodes.Num() ? (BYTE)hp2_rs_engine_trace_fast_line_check( &M, &End, &Start ) : RootOutside;
	unguard;
}

/*---------------------------------------------------------------------------------------
   LineCheck.
---------------------------------------------------------------------------------------*/

//
// Try moving a collision box from Start to End and see what it collides
// with. Returns 1 if unblocked, 0 if blocked.
//
// Note: This function assumes that Start is a valid starting point which
// is not stuck in solid space (i.e. CollisionPointCheck(Start)=1). If Start is
// stuck in solid space, the returned time will be meaningless.
//
UBOOL UModel::LineCheck
(
	FCheckResult&	Hit,
	AActor*			Owner,
	FVector			End,
	FVector			Start,
	FVector			Extent,
	DWORD           ExtraNodeFlags
)
{
	guard(UModel::LineCheck);
	Hp2TraceModelArg M;
	Hp2TraceModelFor( this, M );
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_trace_model_line_check( &Hit, &M, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), &End, &Start, &Extent, ExtraNodeFlags );
	unguard;
}

/*---------------------------------------------------------------------------------------
   Region determination.
---------------------------------------------------------------------------------------*/

//
// Figure out which zone a point is in, and return it.  A value of
// zero indicates that the point doesn't fall into any zone.
//
FPointRegion UModel::PointRegion( AZoneInfo* Zone, FVector Location ) const
{
	guard(UModel::PointRegion);
	check(Zone!=NULL);

	FPointRegion Result( Zone, INDEX_NONE, 0 );
	if( Nodes.Num() ) 
	{
		Hp2TraceModelArg M;
		Hp2TraceModelFor( this, M );
		INT   iLeaf      = INDEX_NONE;
		BYTE  ZoneNumber = 0;
		hp2_rs_engine_trace_point_region( &M, &Location, &iLeaf, &ZoneNumber );
		Result.iLeaf      = iLeaf;
		Result.ZoneNumber = ZoneNumber;
		Result.Zone       = Zones[Result.ZoneNumber].ZoneActor ? Zones[Result.ZoneNumber].ZoneActor : Zone;
	}
	return Result;
	unguard;
}

/*---------------------------------------------------------------------------------------
   Point searching.
---------------------------------------------------------------------------------------*/

//
// Find Bsp node vertex nearest to a point (within a certain radius) and
// set the location.  Returns distance, or -1.f if no point was found.
//
FLOAT UModel::FindNearestVertex
(
	const FVector	&SourcePoint,
	FVector			&DestPoint,
	FLOAT			MinRadius, 
	INT				&pVertex
) const
{
	guard(UModel::FindNearestVertex);
	Hp2TraceModelArg M;
	Hp2TraceModelFor( this, M );
	return hp2_rs_engine_trace_find_nearest_vertex( &M, &SourcePoint, &DestPoint, MinRadius, &pVertex );
	unguard;
}

/*---------------------------------------------------------------------------------------
   Bound filter precompute.
---------------------------------------------------------------------------------------*/

//
// Precompute the front/back test for a bounding sphere.  Tags all nodes that
// the sphere falls into with a NF_IsBack tag (if the sphere is entirely in back
// of the node), a NF_IsFront tag (if the sphere is entirely in front of the node),
// or neither (if the sphere is split by the node).  This only affects nodes
// that the sphere falls in.  Thus, it is not necessary to perform any cleanup
// after precomputing the filter as long as you're sure the sphere completely
// encloses the object whose filter you're precomputing.
//
void UModel::PrecomputeSphereFilter( const FPlane& Sphere )
{
	guard(UModel::PrecomputeSphereFilter);

	if( Nodes.Num() )
		hp2_rs_engine_trace_precompute_sphere_filter( &Nodes(0), Nodes.Num(), &Sphere );

	unguard;
}

/*---------------------------------------------------------------------------------------
   The End.
---------------------------------------------------------------------------------------*/
