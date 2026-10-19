/*=============================================================================
	UnRoute.cpp: Unreal AI routing code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* ...

	The routing algorithms are ported to Rust (crates/hp2-engine/src/route.rs).
	These are thin forwarders; the services they call are in HP2RouteServices.cpp.
=============================================================================*/

#include "EnginePrivate.h"
#include "HP2RouteServices.h"

static_assert( sizeof(FSortedPathList) == sizeof(Hp2SortedPathList), "FSortedPathList must match hp2-engine::route::Hp2SortedPathList" );

void APawn::clearPaths()
{
	guard(APawn::clearPaths);
	hp2_rs_engine_route_clear_paths( &GHp2RouteServices, this );
	unguard;
}

void FSortedPathList::FindVisiblePaths(APawn *Searcher, FVector Dest, FSortedPathList *DestPoints, INT bClearPaths,
							 INT &startanchor, INT &endanchor)
{
	guard(FSortedPathList::FindVisiblePaths);
	hp2_rs_engine_route_sorted_find_visible_paths( (Hp2SortedPathList*)this, &GHp2RouteServices, Searcher, &Dest,
		(Hp2SortedPathList*)DestPoints, bClearPaths, &startanchor, &endanchor );
	unguard;
}

int FSortedPathList::findEndPoint(APawn *Searcher, INT &startanchor)
{
	guard(FSortedPathList::findEndPoint);
	return hp2_rs_engine_route_sorted_find_end_point( (Hp2SortedPathList*)this, &GHp2RouteServices, Searcher, &startanchor );
	unguard;
}

int FSortedPathList::checkAnchorPath(APawn *Searcher, FVector Dest)
{
	guard(FSortedPathList::checkAnchorPath);
	return hp2_rs_engine_route_sorted_check_anchor_path( (Hp2SortedPathList*)this, &GHp2RouteServices, Searcher, &Dest );
	unguard;
}

void FSortedPathList::expandAnchor(APawn *Searcher)
{
	guard(FSortedPathList::expandAnchor);
	hp2_rs_engine_route_sorted_expand_anchor( (Hp2SortedPathList*)this, &GHp2RouteServices, Searcher );
	unguard;
}

int APawn::CanMoveTo(AActor *Anchor, AActor *Dest)
{
	guard(APawn::CanMoveTo);
	return hp2_rs_engine_route_can_move_to( &GHp2RouteServices, this, Anchor, Dest );
	unguard;
}

void FSortedPathList::findAltEndPoint(APawn *Searcher, AActor *&bestPath)
{
	guard(FSortedPathList::findAltEndPoint);
	hp2_rs_engine_route_sorted_find_alt_end_point( (Hp2SortedPathList*)this, &GHp2RouteServices, Searcher, (void**)&bestPath );
	unguard;
}

int APawn::findPathToward(AActor *goal, INT bSinglePath, AActor *&bestPath, INT bClearPaths)
{
	guard(APawn::findPathToward);
	return hp2_rs_engine_route_find_path_toward( &GHp2RouteServices, this, goal, bSinglePath, (void**)&bestPath, bClearPaths );
	unguard;
}

FLOAT APawn::findPathTowardBestInventory(AActor *&bestPath, INT bClearPaths, FLOAT MinWeight, INT bPredictRespawns)
{
	guard(APawn::findPathTowardBestInventory);
	return hp2_rs_engine_route_find_path_toward_best_inventory( &GHp2RouteServices, this, (void**)&bestPath, bClearPaths, MinWeight, bPredictRespawns );
	unguard;
}

bool APawn::findPath(ANavigationPoint* &next, AActor* startPoint, FName DestName)
{
	guard(APawn::findPath);
	return hp2_rs_engine_route_find_path( &GHp2RouteServices, this, (void**)&next, startPoint, DestName.GetIndex() ) != 0;
	unguard;
}

void APawn::givePathEnds(int whichRS, AActor * endA, AActor * endB)
{
	FReachSpec* spec;
	guard(APawn::givePathEnds);
	spec=&GetLevel()->ReachSpecs(whichRS);
	endA=spec->Start;
	endB=spec->End;
	unguard;

}

int APawn::findPathTo(FVector Dest, INT bSinglePath, AActor *&bestPath, INT bClearPaths)
{
	guard(APawn::findPathTo);
	return hp2_rs_engine_route_find_path_to( &GHp2RouteServices, this, &Dest, bSinglePath, (void**)&bestPath, bClearPaths );
	unguard;
}

int APawn::findRandomDest(AActor *&bestPath)
{
	guard(APawn::findRandomDest);
	return hp2_rs_engine_route_find_random_dest( &GHp2RouteServices, this, (void**)&bestPath );
	unguard;
}

int APawn::TraverseFrom(AActor *start, int moveFlags)
{
	guard(APawn::TraverseFrom);
	return hp2_rs_engine_route_traverse_from( &GHp2RouteServices, this, start, moveFlags );
	unguard;
}

int APawn::breadthPathFrom(AActor *start, AActor *&bestPath, int bSinglePath, int moveFlags)
{
	guard(APawn::breadthPathFrom);
	return hp2_rs_engine_route_breadth_path_from( &GHp2RouteServices, this, start, (void**)&bestPath, bSinglePath, moveFlags );
	unguard;
}

void FSortedPathList::addPath(AActor * node, INT dist)
{
	guard(FSortedPathList::addPath);
	hp2_rs_engine_route_sorted_add_path( (Hp2SortedPathList*)this, node, dist );
	unguard;
}

void FSortedPathList::removePath(int p)
{
	guard(FSortedPathList::removePath);
	hp2_rs_engine_route_sorted_remove_path( (Hp2SortedPathList*)this, p );
	unguard;
}

FLOAT APawn::breadthPathToInventory(AActor *start, AActor *&bestPath, int moveFlags, FLOAT bestInventoryWeight, INT bPredictRespawns)
{
	guard(APawn::breadthPathToInventory);
	return hp2_rs_engine_route_breadth_path_to_inventory( &GHp2RouteServices, this, start, (void**)&bestPath, moveFlags, bestInventoryWeight, bPredictRespawns );
	unguard;
}

void APawn::SetRouteCache(ANavigationPoint *BestPath)
{
	guard(APawn::SetRouteCache);
	hp2_rs_engine_route_set_route_cache( this, BestPath );
	unguard;
}

void APawn::ReverseRouteFor(ANavigationPoint *BestDest)
{
	guard(APawn::ReverseRouteFor);
	hp2_rs_engine_route_reverse_route_for( BestDest );
	unguard;
}
