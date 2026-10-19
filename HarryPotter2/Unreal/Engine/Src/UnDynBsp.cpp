/*=============================================================================
	UnDynBsp.cpp: Unreal dynamic Bsp object support
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	This code handles all moving brushes with a level.  These are objects which move
	once in a while and are added to and maintained within the level's Bsp whenever they 
	move.

Revision history:
	* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2DynBspServices.h"

/*---------------------------------------------------------------------------------------
	Brush class implementation.
---------------------------------------------------------------------------------------*/

void ABrush::InitPosRotScale()
{
	guard(ABrush::InitPosRotScale);
	check(Brush);
	
	MainScale = GMath.UnitScale;
	PostScale = GMath.UnitScale;
	Location  = FVector(0,0,0);
	Rotation  = FRotator(0,0,0);
	PrePivot  = FVector(0,0,0);

	unguard;
}
void ABrush::PostLoad()
{
	guard(ABrush::PostLoad);
	Super::PostLoad();
	unguard;
}
IMPLEMENT_CLASS(ABrush);

/*---------------------------------------------------------------------------------------
	Globals.
---------------------------------------------------------------------------------------*/

//
// Tracks moving brushes within a level.  One of these structures is kept for each 
// active level.  The structure is not saved with levels, but is rather rebuild at 
// load-time.  The implementation lives in Rust (crates/hp2-engine/src/dynbsp.rs);
// this class only owns the handle.
//
class FMovingBrushTracker : public FMovingBrushTrackerBase
{
public:
	// FMovingBrushTrackerBase interface.
	FMovingBrushTracker( ULevel *ThisLevel );
	~FMovingBrushTracker();
	void Flush( AActor *Actor );
	void Update( AActor *Actor );
	void CountBytes( FArchive& Ar );
	UBOOL SurfIsDynamic( INT iSurf );

private:
	void* Rust;	// Rust DynBsp<CHost> handle.
};

FMovingBrushTracker::FMovingBrushTracker( ULevel* ThisLevel )
{
	guard(FMovingBrushTracker::FMovingBrushTracker);
	Rust = hp2_rs_dynbsp_new( &GHp2DynBspServices, ThisLevel, ThisLevel->Model );
	unguard;
}

FMovingBrushTracker::~FMovingBrushTracker()
{
	guard(FMovingBrushTracker::~FMovingBrushTracker);
	hp2_rs_dynbsp_free( Rust );
	unguard;
}

void FMovingBrushTracker::Flush( AActor* Actor )
{
	guard(FMovingBrushTracker::Flush);
	hp2_rs_dynbsp_flush( Rust, Actor );
	unguard;
}

void FMovingBrushTracker::Update( AActor* Actor )
{
	guard(FMovingBrushTracker::Update);
	hp2_rs_dynbsp_update( Rust, Actor );
	unguard;
}

UBOOL FMovingBrushTracker::SurfIsDynamic( INT iSurf )
{
	guard(FMovingBrushTracker::SurfIsDynamic);
	return hp2_rs_dynbsp_surf_is_dynamic( Rust, iSurf ) != 0;
	unguard;
}

void FMovingBrushTracker::CountBytes( FArchive& Ar )
{
	guard(FMovingBrushTracker::CountBytes);
	hp2_rs_dynbsp_count_bytes( Rust, &GHp2DynBspServices, &Ar );
	unguard;
}

/*---------------------------------------------------------------------------------------
	Instantiation.
---------------------------------------------------------------------------------------*/

ENGINE_API FMovingBrushTrackerBase* GNewBrushTracker( ULevel* InLevel )
{
	guard(GNewBrushTracker);
	return new(TEXT("FMovingBrushTracker"))FMovingBrushTracker( InLevel );
	unguard;
}

/*---------------------------------------------------------------------------------------
	The End.
---------------------------------------------------------------------------------------*/
