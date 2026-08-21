/*=============================================================================
	UnPrim.h: Unreal UPrimitive definition.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

/*-----------------------------------------------------------------------------
	Forward declarations.
-----------------------------------------------------------------------------*/

class UTexture;
class URenderBase;
struct FSceneNode;

struct FCheckResult;
QSORT_RETURN CDECL CompareHits( const FCheckResult* A, const FCheckResult* B );
/*-----------------------------------------------------------------------------
	FCheckResult.
-----------------------------------------------------------------------------*/

//
// Results of an actor check.
//
struct FIteratorActorList : public FIteratorList
{
	// Variables.
	AActor* Actor;

	// Functions.
	FIteratorActorList()
	{}
	FIteratorActorList( FIteratorActorList* InNext, AActor* InActor )
	:	FIteratorList	(InNext)
	,	Actor			(InActor)
	{}
	FIteratorActorList* GetNext()
	{ return (FIteratorActorList*) Next; }
};

//
// Results from a collision check.
//
struct FCheckResult : public FIteratorActorList
{
	// Variables.
	FVector		Location;   // Location of the hit in coordinate system of the returner.
	FVector		Normal;     // Normal vector in coordinate system of the returner. Zero=none.
	UPrimitive*	Primitive;  // Actor primitive which was hit, or NULL=none.
	FLOAT       Time;       // Time until hit, if line check.
	INT			Item;       // Primitive data item which was hit, INDEX_NONE=none.

	// Functions.
	FCheckResult()
	{}
	FCheckResult( FLOAT InTime, FCheckResult* InNext=NULL )
	:	FIteratorActorList( InNext, NULL )
	,	Location	(0,0,0)
	,	Normal		(0,0,0)
	,	Primitive	(NULL)
	,	Time		(InTime)
	,	Item		(INDEX_NONE)
	{}
	FCheckResult*& GetNext()
		{ return *(FCheckResult**)&Next; }
	friend QSORT_RETURN CDECL CompareHits( const FCheckResult* A, const FCheckResult* B )
		{ return A->Time<B->Time ? -1 : A->Time>B->Time ? 1 : 0; }

	// Return geometry data. Requires some additional computation, as the Item node index
	// indicates only the first node in a coplanar set..
	FBspNode* BspNode( FLOAT Radius = 0.f ) const;
	FBspSurf* BspSurf( FLOAT Radius = 0.f ) const;
	//
	// Returns a pointer to the hit texture. If the pointer is null, a valid
	// texture cannot be found. If no parameter is specified, the level class is
	// assumed.
	//
	UTexture* Texture( class UClass* SomeBase = 0 ) const;
};

/*-----------------------------------------------------------------------------
	UPrimitive.
-----------------------------------------------------------------------------*/

//
// UPrimitive, the base class of geometric entities capable of being
// rendered and collided with.
// For historical reasons, the base primitive class implements a vertical 
// cylinder geometry, but contains fields for a bounding box and sphere,
// used by derived classes.
//
class ENGINE_API UPrimitive : public UObject
{
	DECLARE_CLASS(UPrimitive,UObject,0,Engine)

	// Variables.
	FBox BoundingBox;
	FSphere BoundingSphere;

	// Constructor.
	UPrimitive()
	: BoundingBox(0)
	, BoundingSphere(0)
	{}

	// UObject interface.
	void Serialize( FArchive& Ar );

	// UPrimitive collision interface.
	virtual UBOOL PointCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			Location,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual UBOOL LineCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			End,
		FVector			Start,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	virtual FVector GetCollisionExtent( const AActor* Owner ) const;		// Local max extents.
	virtual void DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags );
	virtual void ValidateActor( AActor* Owner );
};

//
// Inherits all of UPrimitive's basic cylinder behaviour.
//
typedef UPrimitive UCylinder;

/*-----------------------------------------------------------------------------
	UOrientedCylinder. As UCylinder, but takes Owner's orientation.
-----------------------------------------------------------------------------*/

class ENGINE_API UOrientedCylinder: public UPrimitive
{
	DECLARE_CLASS(UOrientedCylinder,UPrimitive,0,Engine)

	// UPrimitive collision interface.
	virtual UBOOL PointCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			Location,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual UBOOL LineCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			End,
		FVector			Start,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	virtual void DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags );
};

/*-----------------------------------------------------------------------------
	UAlignedOvalCylinder. As UCylinder, but has two axes: radius and width.
-----------------------------------------------------------------------------*/

class ENGINE_API UAlignedOvalCylinder: public UCylinder
{
	DECLARE_CLASS(UAlignedOvalCylinder,UCylinder,0,Engine)

	// UPrimitive collision interface.
	virtual UBOOL PointCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			Location,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual UBOOL LineCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			End,
		FVector			Start,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	virtual void DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags );
};

/*----------------------------------------------------------------------------------
	UOrientedOvalCylinder. As UAlignedOvalCylinder, but takes Owner's orientation.
----------------------------------------------------------------------------------*/

class ENGINE_API UOrientedOvalCylinder: public UAlignedOvalCylinder
{
	DECLARE_CLASS(UOrientedOvalCylinder,UAlignedOvalCylinder,0,Engine)

	// UPrimitive collision interface.
	virtual UBOOL PointCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			Location,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual UBOOL LineCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			End,
		FVector			Start,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	virtual void DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags );
};

/*-----------------------------------------------------------------------------
	UBox. Oriented box defined by owner Collision params.
-----------------------------------------------------------------------------*/

class ENGINE_API UBox: public UPrimitive
{
	DECLARE_CLASS(UBox,UPrimitive,0,Engine)

	// UPrimitive collision interface.
	virtual UBOOL PointCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			Location,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual UBOOL LineCheck
	(
		FCheckResult	&Result,
		AActor			*Owner,
		FVector			End,
		FVector			Start,
		FVector			Extent,
		DWORD           ExtraNodeFlags
	);
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	virtual FVector GetCollisionExtent( const AActor* Owner ) const;		// Local max extents.
	virtual void DrawCollisionBounds( AActor* Owner, URenderBase* Render, FSceneNode* Frame, FPlane Color, DWORD LineFlags );
};

/*-----------------------------------------------------------------------------
	UBoxPrim. Oriented box defined by primitive BoundingBox.
-----------------------------------------------------------------------------*/

class ENGINE_API UBoxPrim: public UBox
{
	DECLARE_CLASS(UBoxPrim,UBox,0,Engine)

	// UPrimitive collision interface.
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	virtual FVector GetCollisionExtent( const AActor* Owner ) const;
	virtual void ValidateActor( AActor* Owner );
};


/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/
