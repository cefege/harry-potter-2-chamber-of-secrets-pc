/*=============================================================================
	UnPrim.cpp: Unreal primitive functions.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "HP2PrimTrace.h"

/*----------------------------------------------------------------------------
	UPrimitive object implementation.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UPrimitive);

/*----------------------------------------------------------------------------
	UPrimitive collision checking.
----------------------------------------------------------------------------*/

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
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_prim_cyl_bounds( Hp2PrimOwnerFor(Owner, 0, Arg), bWorld, &Box );
	return Box;
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_cyl_point_check( &Result, Hp2PrimOwnerFor(Owner, 0, Arg), &Location, &Extent );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_cyl_line_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_CYL_EXTENT, Arg), &End, &Start, &Extent );
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
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_prim_oriented_cyl_bounds( Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), bWorld, &Box );
	return Box;
}

void UOrientedCylinder::DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags )
{
	Render->DrawCylinder( Frame, Color, LineFlags, Owner->ToLocal(), Owner->CollisionRadius, Owner->CollisionHeight );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_oriented_cyl_point_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), &Location, &Extent );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_oriented_cyl_line_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES | HP2_OWNER_CYL_EXTENT, Arg), &End, &Start, &Extent );
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
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_prim_aligned_oval_bounds( Hp2PrimOwnerFor(Owner, 0, Arg), bWorld, &Box );
	return Box;
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_aligned_oval_point_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_CYL_ZERO_WIDTH, Arg), &Location, &Extent );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_aligned_oval_line_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_CYL_ZERO_WIDTH, Arg), &End, &Start, &Extent );
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
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_prim_oriented_oval_bounds( Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), bWorld, &Box );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_oriented_oval_point_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES | HP2_OWNER_CYL_ZERO_WIDTH, Arg), &Location, &Extent );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_oriented_oval_line_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES | HP2_OWNER_CYL_ZERO_WIDTH, Arg), &End, &Start, &Extent );
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
	UBox.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UBox);

FBox UBox::GetCollisionBoundingBox(const AActor* Owner, bool bWorld) const 
{
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_prim_ubox_bounds( Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), bWorld, &Box );
	return Box;
}

FVector UBox::GetCollisionExtent(const AActor* Owner) const
{
	Hp2PrimOwnerArg Arg;
	FVector Ext(0,0,0);
	hp2_rs_engine_prim_ubox_extent( Hp2PrimOwnerFor(Owner, 0, Arg), &Ext );
	return Ext;
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_ubox_point_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), &Location, &Extent );
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
	Hp2PrimOwnerArg Arg;
	return hp2_rs_engine_prim_ubox_line_check( &Result, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), &End, &Start, &Extent );
}

/*----------------------------------------------------------------------------
	UBoxPrim.
----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UBoxPrim);

FBox UBoxPrim::GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const
{
	Hp2PrimOwnerArg Arg;
	FBox Box(0);
	hp2_rs_engine_prim_boxprim_bounds( &BoundingBox, Hp2PrimOwnerFor(Owner, HP2_OWNER_FRAMES, Arg), bWorld, &Box );
	return Box;
}

FVector UBoxPrim::GetCollisionExtent( const AActor* Owner ) const
{
	FVector Ext(0,0,0);
	hp2_rs_engine_prim_boxprim_extent( &BoundingBox, &Ext );
	return Ext;
}

void UBoxPrim::ValidateActor( AActor* Owner )
{
	FLOAT Out[2];
	hp2_rs_engine_prim_boxprim_validate( &BoundingBox, Out );
	Owner->CollisionRadius = Owner->CollisionWidth = Out[0];
	Owner->CollisionHeight = Out[1];
}

/*----------------------------------------------------------------------------
	FCheckResult surface identification.
----------------------------------------------------------------------------*/

// Mover hits are reported in mover space; returns the transform to apply, or NULL.
static const FCoords* MoverToLocal( AActor* Actor, FCoords& C )
{
	if( Actor && Actor->IsA(AMover::StaticClass()) )
	{
		C = Actor->ToLocal();
		return &C;
	}
	return NULL;
}

FBspNode* FCheckResult::BspNode( FLOAT Radius ) const
{
	guard(FCheckResult::BspNode);

	// Only model hits have nodes.
	UModel* Model = Cast<UModel>(Primitive);
	if( !Model )
		return NULL;

	FCoords C;
	Hp2BspArgs Bsp;
	Hp2BspArgsFor( Model, Bsp );
	INT iNode = hp2_rs_engine_prim_bsp_node( &Bsp, Item, &Location, &Normal, Radius, MoverToLocal(Actor, C) );
	return iNode == INDEX_NONE ? NULL : &Model->Nodes(iNode);
	unguard;
}

FBspSurf* FCheckResult::BspSurf( FLOAT Radius ) const
{
	// Only model hits have surfaces.
	UModel* Model = Cast<UModel>(Primitive);
	if( !Model )
		return NULL;

	FCoords C;
	Hp2BspArgs Bsp;
	Hp2BspArgsFor( Model, Bsp );
	INT iSurf = hp2_rs_engine_prim_bsp_surf( &Bsp, Item, &Location, &Normal, Radius, MoverToLocal(Actor, C) );
	return iSurf == INDEX_NONE ? NULL : &Model->Surfs(iSurf);
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
