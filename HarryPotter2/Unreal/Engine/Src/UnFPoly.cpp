/*=============================================================================
	UnFPoly.cpp: FPoly implementation (Editor polygons).
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2GeometryServices.h"

/*---------------------------------------------------------------------------------------
	FPoly class implementation.

	The polygon algorithms live in Rust (crates/hp2-engine/src/fpoly.rs); these
	methods forward to the hp2_rs_engine_fpoly_* exports. Rust returns fatal
	messages through an error buffer; FPolyFatal raises them with appErrorf,
	matching the original fatal paths.
---------------------------------------------------------------------------------------*/

enum { FPOLY_ERR_CAP = 256 };

// Raises a fatal error reported by the Rust side (empty buffer = no error).
static void FPolyFatal( const char* Err )
{
	if( Err[0] )
		appErrorf( TEXT("%s"), ANSI_TO_TCHAR(Err) );
}

//
// Initialize everything in an  editor polygon structure to defaults.
//
void FPoly::Init()
{
	guard(FPoly::Init);

	Base			= FVector(0,0,0);
	Normal			= FVector(0,0,0);
	TextureU		= FVector(0,0,0);
	TextureV		= FVector(0,0,0);
	PolyFlags       = 0;
	Actor			= NULL;
	Texture         = NULL;
	ItemName        = NAME_None;
	NumVertices     = 0;
	iLink           = INDEX_NONE;
	iBrushPoly		= INDEX_NONE;
	PanU			= 0;
	PanV			= 0;
	SavePolyIndex	= -1;
	bFaceDragSel	= 0;

	unguard;
}

//
// Reverse an FPoly by revesing the normal and reversing the order of its
// vertices.
//
void FPoly::Reverse()
{
	guard(FPoly::Reverse);

	hp2_rs_engine_fpoly_reverse( this );

	unguard;
}

//
// Fix up an editor poly by deleting vertices that are identical.  Sets
// vertex count to zero if it collapses.  Returns number of vertices, 0 or >=3.
//
int FPoly::Fix()
{
	guard(FPoly::Fix);

	return hp2_rs_engine_fpoly_fix( &GHp2GeometryServices, this );

	unguard;
}

//
// Compute the 2D area.
//
FLOAT FPoly::Area()
{
	guard(FPoly::Area);

	return hp2_rs_engine_fpoly_area( this );

	unguard;
}

//
// Split with plane. Meant to be numerically stable.
//
int FPoly::SplitWithPlane
(
	const FVector	&PlaneBase,
	const FVector	&PlaneNormal,
	FPoly			*FrontPoly,
	FPoly			*BackPoly,
	int				VeryPrecise
) const
{
	guard(FPoly::SplitWithPlane);

	check(NumVertices>=3);
	check(NumVertices<=MAX_VERTICES);

	char Err[FPOLY_ERR_CAP] = {0};
	int Result = hp2_rs_engine_fpoly_split_with_plane
	(
		&GHp2GeometryServices,
		this,
		&PlaneBase,
		&PlaneNormal,
		FrontPoly,
		BackPoly,
		VeryPrecise,
		Err,
		FPOLY_ERR_CAP
	);
	FPolyFatal( Err );
	return Result;

	unguard;
}

//
// Split with a Bsp node.
//
int FPoly::SplitWithNode
(
	const UModel	*Model,
	INT				iNode,
	FPoly			*FrontPoly,
	FPoly			*BackPoly,
	INT				VeryPrecise
) const
{
	guard(FPoly::SplitWithNode);
	const FBspNode &Node = Model->Nodes(iNode       );
	const FBspSurf &Surf = Model->Surfs(Node.iSurf  );

	return SplitWithPlane
	(
		Model->Points (Surf.pBase  ),
		Model->Vectors(Surf.vNormal),
		FrontPoly, 
		BackPoly, 
		VeryPrecise
	);
	unguard;
}

//
// Split with plane quickly for in-game geometry operations.
// Results are always valid. May return sliver polys.
//
int FPoly::SplitWithPlaneFast
(
	const FPlane	Plane,
	FPoly*			FrontPoly,
	FPoly*			BackPoly
) const
{
	guard(FPoly::SplitWithPlaneFast);

	char Err[FPOLY_ERR_CAP] = {0};
	int Result = hp2_rs_engine_fpoly_split_with_plane_fast
	(
		this,
		&Plane,
		FrontPoly,
		BackPoly,
		Err,
		FPOLY_ERR_CAP
	);
	FPolyFatal( Err );
	return Result;

	unguard;
}

//
// Split an FPoly in half.
//
void FPoly::SplitInHalf( FPoly *OtherHalf )
{
	guard(FPoly::SplitInHalf);

	char Err[FPOLY_ERR_CAP] = {0};
	hp2_rs_engine_fpoly_split_in_half( this, OtherHalf, Err, FPOLY_ERR_CAP );
	FPolyFatal( Err );

	unguard;
}

//
// Compute normal of an FPoly.  Works even if FPoly has 180-degree-angled sides (which
// are often created during T-joint elimination).  Returns nonzero result (plus sets
// normal vector to zero) if a problem occurs.
//
int FPoly::CalcNormal( UBOOL bSilent )
{
	guard(FPoly::CalcNormal);

	return hp2_rs_engine_fpoly_calc_normal( &GHp2GeometryServices, this, bSilent );

	unguard;
}

//
// Transform an editor polygon with a coordinate system, a pre-transformation
// addition, and a post-transformation addition:
//
void FPoly::Transform
(
	const FModelCoords&	Coords,
	const FVector&		PreSubtract,
	const FVector&		PostAdd,
	FLOAT				Orientation
)
{
	guard(FPoly::Transform);

	hp2_rs_engine_fpoly_transform
	(
		this,
		&Coords.PointXform,
		&Coords.VectorXform,
		&PreSubtract,
		&PostAdd,
		Orientation
	);

	unguard;
}

//
// Remove colinear vertices and check convexity.  Returns 1 if convex, 0 if
// nonconvex or collapsed.
//
INT FPoly::RemoveColinears()
{
	guard(FPoly::RemoveColinears);

	return hp2_rs_engine_fpoly_remove_colinears( this );

	unguard;
}

//
// Split a poly and keep only the front half. Returns number of vertices,
// 0 if clipped away.
//
int FPoly::Split( const FVector &Normal, const FVector &Base, int NoOverflow )
{
	guard(FPoly::Split);

	char Err[FPOLY_ERR_CAP] = {0};
	int Result = hp2_rs_engine_fpoly_split( this, &Normal, &Base, NoOverflow, Err, FPOLY_ERR_CAP );
	FPolyFatal( Err );
	return Result;

	unguard;
}

//
// Compute all remaining polygon parameters (normal, etc) that are blank.
// Returns 0 if ok, nonzero if problem.
//
int FPoly::Finalize( int NoError )
{
	guard(FPoly::Finalize);

	char Err[FPOLY_ERR_CAP] = {0};
	int Result = hp2_rs_engine_fpoly_finalize( &GHp2GeometryServices, this, NoError, Err, FPOLY_ERR_CAP );
	FPolyFatal( Err );
	return Result;

	unguard;
}

/*---------------------------------------------------------------------------------------
	FPolys object implementation.
---------------------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UPolys);

/*---------------------------------------------------------------------------------------
	Backfacing.
---------------------------------------------------------------------------------------*/

//
// Return whether this poly and Test are facing each other.
// The polys are facing if they are noncoplanar, one or more of Test's points is in 
// front of this poly, and one or more of this poly's points are behind Test.
//
int FPoly::Faces( const FPoly &Test ) const
{
	guard(FPoly::Faces);

	return hp2_rs_engine_fpoly_faces( this, &Test );

	unguard;
}

/*---------------------------------------------------------------------------------------
	The End.
---------------------------------------------------------------------------------------*/
