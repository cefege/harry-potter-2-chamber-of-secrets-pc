/*=============================================================================
	UnPath.cpp: Unreal pathnode placement

	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	These methods are members of the FPathBuilder class, which adds pathnodes to a level.  
	Paths are placed when the level is built.  FPathBuilder is not used during game play
 
   General comments:
   Path building
   The FPathBuilder does a tour of the level (or a part of it) using the "Scout" actor, starting from
   every actor in the level.  
   This guarantees that correct reachable paths are generated to all actors.  It starts by going to the
   wall to the right of the actor, and following that wall, keeping it to the left.  NOTE: my definition
   of left and right is based on normal Cartesian coordinates, and not screen coordinates (e.g. Y increases
   upward, not downward).  This wall following is accomplished by moving along the wall, constantly looking
   for a leftward passage.  If the FPathBuilder is stopped, he rotates clockwise until he can move forward 
   again.  While performing this tour, it keeps track of the set of previously placed pathnodes which are
   visible and reachable.  If a pathnode is removed from this set, then the FPathBuilder searches for an 
   acceptable waypoint.  If none is found, a new pathnode is added to provide a waypoint back to the no longer
   reachable pathnode.  The tour ends when a full circumlocution has been made, or if the FPathBuilder 
   encounters a previously placed path node going in the same direction.  Pathnodes that were not placed as
   the result of a left turn are marked, since they are due to some possibly unmapped obstruction.  In the
   final phase, these obstructions are inspected.  Then, the pathnode due to the obstruction is removed (since
   the obstruction is now mapped, and any needed waypoints have been built).

  FIXME - ledge and landing marking
  FIXME - mark door centers with left turns (reduces total paths)
  FIXME - paths on top of all platforms
	Revision history:
		* Created by Steven Polge 3/97
=============================================================================*/

#include "EnginePrivate.h"
#include "math.h"

#if DEBUGGINGPATHS
    static inline void CDECL DebugPrint(const TCHAR * Message, ...)
    {
        static int Count = 0;
        if( Count <= 300 ) // To reduce total messages.
        {
            TCHAR Text[4096];
			GET_VARARGS( Text, ARRAY_COUNT(Text), Message );
            debugf( NAME_Log, Text );
        }
    }
    static inline void CDECL DebugVector(const TCHAR *Text, const FVector & Vector )
    {
        TCHAR VectorText[100];
        TCHAR Message[100];
        appSprintf( VectorText, TEXT("[%4.4f,%4.4f,%4.4f]"), Vector.X, Vector.Y, Vector.Z );
        appSprintf( Message, Text, VectorText );
        DebugPrint(Message);
		DebugPrint(VectorText);
    }
	static inline void CDECL DebugFloat(const TCHAR *Text, const FLOAT & val )
    {
        TCHAR VectorText[100];
        TCHAR Message[100];
        appSprintf( VectorText, TEXT("[%4.4f]"), val);
        appSprintf( Message, Text, VectorText );
        DebugPrint(Message);
		DebugPrint(VectorText);
    }
	static inline void CDECL DebugInt(const TCHAR *Text, const int & val )
    {
        TCHAR VectorText[100];
        TCHAR Message[100];
        appSprintf( VectorText, TEXT("[%6d]"), val);
        appSprintf( Message, Text, VectorText );
        DebugPrint(Message);
		DebugPrint(VectorText);
    }
#else
    static inline void CDECL DebugPrint(const TCHAR * Message, ...)
    {
    }
     static inline void CDECL DebugVector(const TCHAR *Text, const FVector & Vector )
    {
    }
	static inline void CDECL DebugFloat(const TCHAR *Text, const FLOAT & val )
    {
    }
	static inline void CDECL DebugInt(const TCHAR *Text, const int & val )
    {
    }
   
#endif

//FIXME: add hiding places, camping and sniping spots (not pathnodes, but other keypoints)
int ENGINE_API FPathBuilder::buildPaths (ULevel *ownerLevel, int optimization)
{
	guard(FPathBuilder::buildPaths);

	FLOAT BuildTime = appSeconds().GetFloat() * -1.f;

	Level = ownerLevel;

	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		AActor *Actor = Level->Actors(i); 
		if (Actor && Actor->IsA(APathNode::StaticClass())
			&& ((APathNode *)Actor)->bAutoBuilt )
		{
			Level->DestroyActor( Actor ); 
		}
	}

	undefinePaths( ownerLevel );
	definePaths( ownerLevel );

	int numpaths = 0;
	getScout();

	Scout->SetCollision(1, 1, 1);
	Scout->bCollideWorld = 1;

	Scout->JumpZ = -1.f; //NO jumping
	Scout->GroundSpeed = 320; 
	Scout->MaxStepHeight = 24; 

	numpaths = numpaths + createPaths(optimization);
	Level->DestroyActor(Scout);

	undefinePaths( ownerLevel );
	definePaths( ownerLevel );

	BuildTime+=appSeconds().GetFloat();
	debugf(TEXT("Total paths build time %f seconds"),BuildTime);

	return numpaths;
	unguard;
}

int ENGINE_API FPathBuilder::removePaths (ULevel *ownerLevel)
{
	guard(FPathBuilder::removePaths);
	Level = ownerLevel;
	int removed = 0;

	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		AActor *Actor = Level->Actors(i); 
		if (Actor && Actor->IsA(APathNode::StaticClass()))
		{
			removed++;
			Level->DestroyActor( Actor ); 
		}
	}
	return removed;
	unguard;
}

int ENGINE_API FPathBuilder::showPaths (ULevel *ownerLevel)
{
	guard(FPathBuilder::showPaths);
	Level = ownerLevel;
	int shown = 0;

	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		AActor *Actor = Level->Actors(i); 
		if (Actor && Actor->IsA(APathNode::StaticClass()))
		{
			shown++;
			Actor->DrawType = DT_Sprite; 
		}
	}
	return shown;
	unguard;
}

int ENGINE_API FPathBuilder::hidePaths (ULevel *ownerLevel)
{
	guard(FPathBuilder::hidePaths);
	Level = ownerLevel;
	int shown = 0;

	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		AActor *Actor = Level->Actors(i); 
		if (Actor && Actor->IsA(APathNode::StaticClass()))
		{
			shown++;
			Actor->DrawType = DT_None; 
		}
	}
	return shown;
	unguard;
}

void ENGINE_API FPathBuilder::undefinePaths (ULevel *ownerLevel)
{
	guard(FPathBuilder::undefinePaths);
	Level = ownerLevel;

	//remove all reachspecs
	debugf(NAME_DevPath,TEXT("Remove %d old reachspecs"), Level->ReachSpecs.Num());
	Level->ReachSpecs.Empty();

	// clear navigationpointlist
	Level->GetLevelInfo()->NavigationPointList = NULL;

	//clear pathnodes
	for (INT i=0; i<Level->Actors.Num(); i++)
	{
#if 1 //U2Ed
		GWarn->StatusUpdatef( i, Level->Actors.Num(), TEXT("%s"), TEXT("Undefining Paths") );
#endif

		AActor *Actor = Level->Actors(i); 
		if (Actor && Actor->IsA(ANavigationPoint::StaticClass()))
		{
			if ( Actor->IsA(AWarpZoneMarker::StaticClass()) || Actor->IsA(ATriggerMarker::StaticClass()) 
					|| Actor->IsA(AInventorySpot::StaticClass()) || Actor->IsA(AButtonMarker::StaticClass()) )
			{
				if ( Actor->IsA(AInventorySpot::StaticClass()) && ((AInventorySpot *)Actor)->markedItem )
					((AInventorySpot *)Actor)->markedItem->MyMarker = NULL;
				Level->DestroyActor(Actor);
			}
			else
			{
				((ANavigationPoint *)Actor)->nextNavigationPoint = NULL;
				((ANavigationPoint *)Actor)->nextOrdered = NULL;
				((ANavigationPoint *)Actor)->prevOrdered = NULL;
				((ANavigationPoint *)Actor)->startPath = NULL;
				((ANavigationPoint *)Actor)->previousPath = NULL;
				for (INT i=0; i<16; i++)
				{
					((ANavigationPoint *)Actor)->Paths[i] = -1;
					((ANavigationPoint *)Actor)->upstreamPaths[i] = -1;
					((ANavigationPoint *)Actor)->PrunedPaths[i] = -1;
					((ANavigationPoint *)Actor)->VisNoReachPaths[i] = NULL;
				}
			}
		}
	}
	unguard;
}

void ENGINE_API FPathBuilder::definePaths (ULevel *ownerLevel)
{
	AActor *Actor1;
	AActor *Actor2;
	float  currentDistance;
	AActor* currentActor;
	float  secondDistance;
	AActor* secondActor;

	const TCHAR* baseName = TEXT("baseStation");
	const TCHAR* className=TEXT("PlayerStart");
	FReachSpec newSpec;
	ANavigationPoint *node;
	Level = ownerLevel;
	Level->GetLevelInfo()->NavigationPointList = NULL;


	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		//GWarn->StatusUpdatef( i, Level->Actors.Num(), TEXT("%s"), TEXT("Defining Paths") );

		Actor1 = Level->Actors(i); 
		Actor2=NULL;
		if ( Actor1 )
		{
			if(Actor1->IsA(ANavigationPoint::StaticClass())&&appStricmp(Actor1->GetClass()->GetName(),className))
			{
				debugf( NAME_DevPath, TEXT("actor 1 is %s"),((Actor1->GetClass()))->GetName() );

				//if(Actor1->GetClass()->GetName()!=baseName)
				if(appStricmp(Actor1->GetClass()->GetName(),baseName))
				{
					

					node=(ANavigationPoint*)Actor1;
					currentActor=NULL;
					currentDistance=300000000;
					secondActor=NULL;
					secondDistance=300000000;
					for (INT ib=0; ib<Level->Actors.Num(); ib++)
					{
						Actor2= Level->Actors(ib); 
						
						if(Actor2 && Actor2!=Actor1)
						{
							if(Actor2->IsA(ANavigationPoint::StaticClass())&&appStricmp(Actor2->GetClass()->GetName(),className))
							{
								debugf( NAME_DevPath, TEXT("actor 2 is %s"),(Actor2->GetClass())->GetName() );

								if(Actor2->GetClass()==Actor1->GetClass()||(appStricmp(Actor2->GetClass()->GetName(),baseName)==0))
								{
									debugf( NAME_DevPath, TEXT("actor2 is %s"),Actor2->GetClass()->GetName() );
									if(currentActor==NULL)
									{
										currentActor=Actor2;
										currentDistance=FDist(currentActor->Location,Actor1->Location);
									}
									else
									{
										if(fabs(FDist(Actor2->Location,Actor1->Location))<currentDistance)
										{
											secondActor=currentActor;
											secondDistance=currentDistance;
											currentActor=Actor2;
											currentDistance=fabs(FDist(currentActor->Location,Actor1->Location));
										}
										else
										{
											if(fabs(FDist(Actor2->Location,Actor1->Location))<secondDistance)
											{
													secondActor=Actor2;
													secondDistance=fabs(FDist(secondActor->Location,Actor1->Location));
											}


										}
									}
								//	if((((ANavigationPoint *)Actor1)->Paths[0] ==-1))
									
								}
								
							}
								
						}
					}
				//	if(Actor1!=Actor2)
					if(currentActor && (((ANavigationPoint*)currentActor)->Paths[2]<0)&&(node->Paths[1]<0))
					{
					//	debugf( NAME_DevPath, TEXT("closest point from %s is %s"),Actor1->GetName(),currentActor->GetName() );
						newSpec.Init();
						newSpec.CollisionRadius = 60;
						newSpec.CollisionHeight = 60;
						newSpec.reachFlags = R_SPECIAL;
						newSpec.Start = node;
						newSpec.End = currentActor;
						newSpec.distance = 500;
					
						int pos = insertReachSpec(node->Paths, newSpec);
					//	debugf( NAME_DevPath, TEXT("pos is %i"),pos );
						if (pos != -1)
						{
							int iSpec = Level->ReachSpecs.AddItem(newSpec);
							node->Paths[pos] = iSpec;
						}
						
						// now tie that one to this one
						newSpec.Init();
						newSpec.CollisionRadius = 60;
						newSpec.CollisionHeight = 60;
						newSpec.reachFlags = R_SPECIAL;
						newSpec.Start = currentActor;
						newSpec.End = node;
						newSpec.distance = 500;
						pos = insertReachSpec(node->Paths, newSpec);
					//	debugf( NAME_DevPath, TEXT("pos is %i"),pos );
						if (pos != -1)
						{
							int iSpec = Level->ReachSpecs.AddItem(newSpec);
							node->Paths[pos] = iSpec;
						}



						if(secondActor!=NULL)
						{
							debugf( NAME_DevPath, TEXT("second closest point from %s is %s"),Actor1->GetName(),currentActor->GetName() );
							newSpec.Init();
							newSpec.CollisionRadius = 60;
							newSpec.CollisionHeight = 60;
							newSpec.reachFlags = R_SPECIAL;
							newSpec.Start = node;
							newSpec.End = secondActor;
							newSpec.distance = 500;
						
							int pos = insertReachSpec(node->Paths, newSpec);
							debugf( NAME_DevPath, TEXT("pos is %i"),pos );
							if (pos != -1)
							{
								int iSpec = Level->ReachSpecs.AddItem(newSpec);
								node->Paths[pos] = iSpec;
							}
					
						// now tie in that one to this one;
							node=(ANavigationPoint*)secondActor;
							debugf( NAME_DevPath, TEXT("second closest point from %s is %s"),Actor1->GetName(),currentActor->GetName() );
							newSpec.Init();
							newSpec.CollisionRadius = 60;
							newSpec.CollisionHeight = 60;
							newSpec.reachFlags = R_SPECIAL;
							newSpec.Start = node;
							newSpec.End = Actor1;
							newSpec.distance = 500;
							pos = insertReachSpec(node->Paths, newSpec);
							debugf( NAME_DevPath, TEXT("pos is %i"),pos );
							if (pos != -1)
							{
								int iSpec = Level->ReachSpecs.AddItem(newSpec);
								node->Paths[pos] = iSpec;
							}
						}

						
					}
						

				}
			}
		}
	}


}
/*
void ENGINE_API FPathBuilder::definePaths (ULevel *ownerLevel)
{
	guard(FPathBuilder::definePaths);
	Level = ownerLevel;
	getScout();
	Level->GetLevelInfo()->NavigationPointList = NULL;

	// Add WarpZoneMarkers and InventorySpots
	debugf( NAME_DevPath, TEXT("Add WarpZone and Inventory markers") );
	for (INT i=0; i<Level->Actors.Num(); i++)
	{
#if 1 //U2Ed
		GWarn->StatusUpdatef( i, Level->Actors.Num(), TEXT("%s"), TEXT("Defining Paths") );
#endif

		AActor *Actor = Level->Actors(i); 
		if ( Actor )
		{
			if ( Actor->IsA(AWarpZoneInfo::StaticClass()) )
			{
				if ( !findScoutStart(Actor->Location) || (Scout->Region.Zone != Actor->Region.Zone) )
				{
					Scout->SetCollisionSize(20, Scout->CollisionHeight);
					if ( !findScoutStart(Actor->Location) || (Scout->Region.Zone != Actor->Region.Zone) )
						Level->FarMoveActor(Scout, Actor->Location, 1, 1);
					Scout->SetCollisionSize(MINCOMMONRADIUS, Scout->CollisionHeight);
				}
				UClass* pathClass = FindObjectChecked<UClass>( ANY_PACKAGE, TEXT("WarpZoneMarker") );
				AWarpZoneMarker *newMarker = (AWarpZoneMarker *)Level->SpawnActor(pathClass, NAME_None, NULL, NULL, Scout->Location);
				newMarker->markedWarpZone = (AWarpZoneInfo *)Actor;
			}
			else if ( Actor->IsA(AInventory::StaticClass()) )
			{
				if ( !findScoutStart(Actor->Location) || (Abs(Scout->Location.Z - Actor->Location.Z) > Scout->CollisionHeight) )
					Level->FarMoveActor(Scout, Actor->Location + FVector(0,0,40 - Actor->CollisionHeight), 1, 1);
				UClass *pathClass = FindObjectChecked<UClass>( ANY_PACKAGE, TEXT("InventorySpot") );
				AInventorySpot *newMarker = (AInventorySpot *)Level->SpawnActor(pathClass, NAME_None, NULL, NULL, Scout->Location);
				newMarker->markedItem = (AInventory *)Actor;
				((AInventory *)Actor)->MyMarker = newMarker;
			}
		}
	}

	//calculate and add reachspecs to pathnodes
	debugf(NAME_DevPath,TEXT("Add reachspecs"));
	for (i=0; i<Level->Actors.Num(); i++)
	{
#if 1 //U2Ed
		GWarn->StatusUpdatef( i, Level->Actors.Num(), TEXT("%s (%d/%d)"), TEXT("Adding reachspecs"), i, Level->Actors.Num());
#endif

		AActor *Actor = Level->Actors(i); 
		if (Actor && !Actor->bDeleteMe && Actor->IsA(ANavigationPoint::StaticClass()))
		{
			((ANavigationPoint *)Actor)->nextNavigationPoint = Level->GetLevelInfo()->NavigationPointList;
			Level->GetLevelInfo()->NavigationPointList = (ANavigationPoint *)Actor;
			addReachSpecs(Actor);
			debugf( NAME_DevPath, TEXT("Added reachspecs to %s"),Actor->GetName() );
		}
	}

#if 1 //U2Ed
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Cleaning up") );
#endif

	debugf(NAME_DevPath,TEXT("Added %d reachspecs"), Level->ReachSpecs.Num()); 
	//remove extra reachspecs from teleporters


	//prune excess reachspecs
	debugf(NAME_DevPath,TEXT("Prune reachspecs"));
	ANavigationPoint *Nav = Level->GetLevelInfo()->NavigationPointList;
	int numPruned = 0;
	while (Nav)
	{
		numPruned += Prune(Nav);
		Nav = Nav->nextNavigationPoint;
	}
	debugf(NAME_DevPath,TEXT("Pruned %d reachspecs"), numPruned);

	// Generate VisNoReach list
	for (ANavigationPoint *Path=Level->GetLevelInfo()->NavigationPointList;
		Path!=NULL;
		Path=Path->nextNavigationPoint)
	{
		addVisNoReach(Path);
	}

	Level->DestroyActor(Scout);
	debugf(NAME_DevPath,TEXT("All done"));
	unguard;
}
*/
//------------------------------------------------------------------------------------------------
//Private methods

/*	if Node is an acceptable waypoint between an upstream path and a downstreampath
	who are also connected, remove their connection
*/
int FPathBuilder::Prune(AActor *Node)
{
	guard(FPathBuilder::Prune);
	
	int pruned = 0;
	ANavigationPoint* NavNode = (ANavigationPoint *)Node;

	int n,j;
	FReachSpec alternatePath, straightPath, part1, part2;
	int i=0;
	while ( (i<16) && (NavNode->upstreamPaths[i] != -1) )
	{
		part1 = Level->ReachSpecs(NavNode->upstreamPaths[i]);
		n=0;
		while ( (n<16) && (NavNode->Paths[n] != -1) )
		{
			part2 = Level->ReachSpecs(NavNode->Paths[n]);
			INT straightPathIndex = specFor(part1.Start, part2.End);
			if (straightPathIndex != -1)
			{
				straightPath = Level->ReachSpecs(straightPathIndex);
				alternatePath = part1 + part2;
				if ( ((float)straightPath.distance * 1.2f >= (float)alternatePath.distance) 
					&& ((alternatePath <= straightPath) || straightPath.BotOnlyPath() 
						|| alternatePath.MonsterPath()) )
				{
					//prune straightpath
					pruned++;
					j=0;
					ANavigationPoint* StartNode = (ANavigationPoint *)(straightPath.Start);
					ANavigationPoint* EndNode = (ANavigationPoint *)(straightPath.End);
					//debugf("Prune reachspec %d from %s to %s because of %s", straightPathIndex,
					//	StartNode->GetName(), EndNode->GetName(), NavNode->GetName()); 
					while ( (j<15) && (StartNode->Paths[j] != straightPathIndex) )
						j++;
					if ( StartNode->Paths[j] == straightPathIndex )
					{
						while ( (j<15) && (StartNode->Paths[j] != -1) )
						{
							StartNode->Paths[j] = StartNode->Paths[j+1];
							j++;
						}
						StartNode->Paths[15] = -1;
					}

					j=0;
					while ( (j<15) && (StartNode->PrunedPaths[j] != -1) )
						j++;
					StartNode->PrunedPaths[j] = straightPathIndex;
					Level->ReachSpecs(straightPathIndex).bPruned = 1;

					j=0;
					while ( (j<15) && (EndNode->upstreamPaths[j] != straightPathIndex) )
						j++;
					if ( EndNode->upstreamPaths[j] == straightPathIndex )
					{
						while ( (j<15) && (EndNode->upstreamPaths[j] != -1) )
						{
							EndNode->upstreamPaths[j] = EndNode->upstreamPaths[j+1];
							j++;
						}
						EndNode->upstreamPaths[15] = -1;
					}
					// Note that all specs remain in reachspec list and still referenced in PrunedPaths[]
					// but removed from visible Navigation graph:
				}
			}
			n++;
		}
		i++;
	}

	return pruned;
	unguard;
}

int FPathBuilder::specFor(AActor* Start, AActor* End)
{
	guard(FPathBuilder::specFor);

	FReachSpec pathSpec;
	ANavigationPoint* StartNode = (ANavigationPoint *)Start;
	int i=0;
	while ( (i<16) && (StartNode->Paths[i] != -1) )
	{
		pathSpec = Level->ReachSpecs(StartNode->Paths[i]);
		if (pathSpec.End == End)
			return StartNode->Paths[i];
		i++;
	}
	return -1;
	unguard;
}

/* insert a reachspec into the array, ordered by longest to shortest distance.
However, if there is no room left in the array, remove longest first
*/
int FPathBuilder::insertReachSpec(INT *SpecArray, FReachSpec &Spec)
{
	guard(FPathBuilder::insertReachSpec);

	int n = 0;
	while ( (n < 16) && (SpecArray[n] != -1) && (Level->ReachSpecs(SpecArray[n]).distance > Spec.distance) )
		n++;
	int pos = n;

	if (SpecArray[15] == -1) //then not full
	{
		if (SpecArray[n] != -1)
		{
			int current = SpecArray[n];
			while (n < 15)
			{
				int next = SpecArray[n+1];
				SpecArray[n+1] = current;
				current = next;
				if (next == -1)
					n = 15;
				n++;
			}
		}
	}
	else if (n == 0) // current is bigger than biggest
	{
		//debugf("Node out of Reachspecs - don't add!");
		return -1;
	}
	else //full, so remove from front
	{
		//debugf("Node out of Reachspecs -add at %d !", pos-1);

		n--;
		pos = n;
		int current = SpecArray[n];
		while (n > 0)
		{
			int next = SpecArray[n-1];
			SpecArray[n-1] = current;
			current = next;
			n--;
		}
	}
	return pos;

	unguard;
}

/* AddVisNoReach()
To the start NavigationPoint's VisNoReach array, add pathnodes which are visible, but not reachable by a direct path,
and are within 3000 units
*/

void FPathBuilder::addVisNoReach(AActor *start)
{
	guard(FPathBuilder::addVisNoReach);

	ANavigationPoint *node = (ANavigationPoint *)start;
	//debugf("Add Reachspecs for node at (%f, %f, %f)", node->Location.X,node->Location.Y,node->Location.Z);

	if ( node->IsA(ALiftCenter::StaticClass()) )
		return;

	Scout->SetCollisionSize( HUMANRADIUS, HUMANHEIGHT );
	Level->FarMoveActor(Scout, node->Location, 1);
	Scout->MoveTarget = node;
	Scout->bCanDoSpecial = 1;
	AActor* newPath;
	INT i = 0;

	//debugf("VisNoReach for %s", node->GetName());
	for (ANavigationPoint *Path=Level->GetLevelInfo()->NavigationPointList;
		Path!=NULL;
		Path=Path->nextNavigationPoint)
	{
		FLOAT distSq = (node->Location - Path->Location).SizeSquared(); 
		if ( !Path->IsA(ALiftCenter::StaticClass()) && (Path != node) 
			&& (distSq < 4000000) && (i < 16) ) //FIXME - Pick right value!!!
		{
			FCheckResult Hit(1.f);
			Level->SingleLineCheck(Hit, Scout, Path->Location, node->Location, TRACE_VisBlocking);
			if ( !Hit.Actor )
			{
				FLOAT pathDist;
				if ( Scout->findPathToward(Path, 0, newPath, 1) )
					pathDist = ((ANavigationPoint *)newPath)->visitedWeight;
				else
				{
					pathDist = 200000000;
					//debugf(TEXT("NO PATH from %s to %s"), node->GetName(), Path->GetName());
				}
				//debugf("Path cost to %s = %f vs. dist %f",Path->GetName(), pathDist, distSq);
				if ( (pathDist != 10000000)
					&& (pathDist * pathDist > 4 * distSq) )
				{
					//debugf("Add %s to %s", Path->GetName(), node->GetName());
					node->VisNoReachPaths[i] = Path;
					i++;
				}
			}
		}
	}
	unguard;
}

/* add reachspecs to path for every path reachable from it. Also add the reachspec to that
paths upstreamPath list
*/
void FPathBuilder::addReachSpecs(AActor *start)
{
	guard(FPathBuilder::addReachspecs);

	FReachSpec newSpec;
	ANavigationPoint *node = (ANavigationPoint *)start;
	//debugf("Add Reachspecs for node at (%f, %f, %f)", node->Location.X,node->Location.Y,node->Location.Z);

	if ( node->IsA(ALiftCenter::StaticClass()) )
	{
		FName myLiftTag = ((ALiftCenter *)node)->LiftTag;
		for (INT i=0; i<Level->Actors.Num(); i++)
		{
			AActor *Actor = Level->Actors(i); 
			if ( Actor && Actor->IsA(ALiftExit::StaticClass()) && (((ALiftExit *)Actor)->LiftTag == myLiftTag) ) 
			{
				newSpec.Init();
				newSpec.CollisionRadius = 60;
				newSpec.CollisionHeight = 60;
				newSpec.reachFlags = R_SPECIAL;
				newSpec.Start = node;
				newSpec.End = Actor;
				newSpec.distance = 500;
				int pos = insertReachSpec(node->Paths, newSpec);
				if (pos != -1)
				{
					int iSpec = Level->ReachSpecs.AddItem(newSpec);
					node->Paths[pos] = iSpec;
					pos = insertReachSpec(((ANavigationPoint *)Actor)->upstreamPaths, newSpec);
					if (pos != -1)
						((ANavigationPoint *)Actor)->upstreamPaths[pos] = iSpec;
				}
				newSpec.Init();
				newSpec.CollisionRadius = 60;
				newSpec.CollisionHeight = 60;
				newSpec.reachFlags = R_SPECIAL;
				newSpec.Start = Actor;
				newSpec.End = node;
				newSpec.distance = 500;
				pos = insertReachSpec(((ANavigationPoint *)Actor)->Paths, newSpec);
				if (pos != -1)
				{
					int iSpec = Level->ReachSpecs.AddItem(newSpec);
					((ANavigationPoint *)Actor)->Paths[pos] = iSpec;
					pos = insertReachSpec(node->upstreamPaths, newSpec);
					if (pos != -1)
						node->upstreamPaths[pos] = iSpec;
				}
			}
		}
		return;
	}

	if ( node->IsA(ATeleporter::StaticClass()) || node->IsA(AWarpZoneMarker::StaticClass()) )
	{
		for (INT i=0; i<Level->Actors.Num(); i++)
		{
			int bFoundMatch = 0;
			AActor *Actor = Level->Actors(i); 
			if ( node->IsA(ATeleporter::StaticClass()) )
			{
				if ( Actor && Actor->IsA(ATeleporter::StaticClass()) && (Actor != node) ) 
					bFoundMatch = (((ATeleporter *)node)->URL==*Actor->Tag);
			}
			else if ( Actor && Actor->IsA(AWarpZoneMarker::StaticClass()) && (Actor != node) )
				bFoundMatch = (((AWarpZoneMarker *)node)->markedWarpZone->OtherSideURL==*((AWarpZoneMarker *)Actor)->markedWarpZone->ThisTag);

			if ( bFoundMatch )
			{
				newSpec.Init();
				newSpec.CollisionRadius = 150;
				newSpec.CollisionHeight = 150;
				newSpec.reachFlags = R_SPECIAL;
				newSpec.Start = node;
				newSpec.End = Actor;
				newSpec.distance = 100;
				int pos = insertReachSpec(node->Paths, newSpec);
				if (pos != -1)
				{
					int iSpec = Level->ReachSpecs.AddItem(newSpec);
					//debugf("     Add teleport reachspec %d to node at (%f, %f, %f)", iSpec, Actor->Location.X,Actor->Location.Y,Actor->Location.Z);
					node->Paths[pos] = iSpec;
					pos = insertReachSpec(((ANavigationPoint *)Actor)->upstreamPaths, newSpec);
					if (pos != -1)
						((ANavigationPoint *)Actor)->upstreamPaths[pos] = iSpec;
				}
				break;
			}
		}
	}

	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		AActor *Actor = Level->Actors(i); 
		if (Actor && !Actor->bDeleteMe && Actor->IsA(ANavigationPoint::StaticClass()) && !Actor->IsA(ALiftCenter::StaticClass()) 
				&& (Actor != node) && ((node->Location - Actor->Location).SizeSquared() < 1000000)
				&& (!node->bOneWayPath || (((Actor->Location - node->Location) | node->Rotation.Vector()) > 0)) )
		{
			if ( (Actor->Location - node->Location).SizeSquared() < 1000 )
				debugf(TEXT("WARNING: %s and %s may be too close!"), Actor->GetName(), node->GetName());
			newSpec.Init();
			if (newSpec.defineFor(node, Actor, Scout))
			{
				int pos = insertReachSpec(node->Paths, newSpec);
				if (pos != -1)
				{
					int iSpec = Level->ReachSpecs.AddItem(newSpec);
					//debugf("     Add reachspec %d to node at (%f, %f, %f)", iSpec, Actor->Location.X,Actor->Location.Y,Actor->Location.Z);
					node->Paths[pos] = iSpec;
					pos = insertReachSpec(((ANavigationPoint *)Actor)->upstreamPaths, newSpec);
					if (pos != -1)
						((ANavigationPoint *)Actor)->upstreamPaths[pos] = iSpec;
				} 
			}
		}
	}
	unguard;
}

/* createPaths()
build paths for a given pawn type (which Scout is emulating)
*/
int FPathBuilder::createPaths (int optimization)
{
	guard(FPathBuilder::createPaths);

	// make sure all navigation points are cleared
	for ( INT i=0; i<Level->Actors.Num(); i++ ) 
	{
		ANavigationPoint *Nav = (ANavigationPoint *)(Level->Actors(i));
		if ( Nav && !Nav->bDeleteMe && Nav->IsA(ANavigationPoint::StaticClass()) )
		{
			Nav->visitedWeight = 1;
			Nav->bEndPoint = 0;
		}
	}

	// multi pass build paths from every playerstart or inventory position in level
	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		ANavigationPoint *Nav = (ANavigationPoint *)(Level->Actors(i));
		if ( Nav && (Nav->IsA(AInventorySpot::StaticClass()) || Nav->IsA(APlayerStart::StaticClass())) )
		{
			debugf(TEXT("----------------------Starting From %s"), Nav->GetName());
			// check if this inventory has already been visited
			if ( !Nav->bEndPoint )
				testPathsFrom(Nav->Location); 
			else
				debugf(TEXT("%s already visited!"),Nav->GetName());
		}
	}

	// merge paths which are within 128 units of each other and can be merged
	Scout->SetCollisionSize(COMMONRADIUS, MINCOMMONHEIGHT);
	for ( INT i=0; i<Level->Actors.Num(); i++ )
	{
		ANavigationPoint *node = (ANavigationPoint *)(Level->Actors(i)); 
		if (node && !node->bDeleteMe && node->bAutoBuilt && node->IsA(ANavigationPoint::StaticClass()) )
			for (INT j=0; j<Level->Actors.Num(); j++)
			{
				ANavigationPoint *node2 = (ANavigationPoint *)(Level->Actors(j)); 
				if ( ValidNode(node, node2) 
					&& ((node->Location - node2->Location).SizeSquared() < 16384)
						&& (!node->bOneWayPath || (((node2->Location - node->Location) | node->Rotation.Vector()) > 0)) 
						&& Level->Model->FastLineCheck(node->Location, node2->Location) )
				{
					debugf(TEXT("Found potential merge pair %s and %s"),node->GetName(), node2->GetName());
					// see if scout can walk without jumping node to node2
					if ( TestReach(node->Location, node2->Location) ) 
					{
						//check if same reachability
						// see if there is an intermediate path
						INT bFoundDiff = 0;
						INT bMustNotAvg = 0;
						FVector AvgPos = 0.5 * (node->Location + node2->Location);
						for (INT k=0; k<Level->Actors.Num(); k++)
						{
							ANavigationPoint *node3 = (ANavigationPoint *)(Level->Actors(k)); 
							if ( ValidNode(node,node3) 
									&& (node3 != node2) && ((node->Location - node3->Location).SizeSquared() < 640000)
									&& ((node2->Location - node3->Location).SizeSquared() < 640000) )
							{
								Scout->SetCollisionSize(COMMONRADIUS, MINCOMMONHEIGHT);
								if ( Level->Model->FastLineCheck(node->Location, node3->Location) )
								{
									if ( TestReach(node->Location, node3->Location) ) 
										bFoundDiff = 1;
									else
									{
										Scout->SetCollisionSize(MINCOMMONRADIUS, MINCOMMONHEIGHT);
										if ( TestReach(node->Location, node3->Location) ) 
											bFoundDiff = 1;
									}
								}
								if ( bFoundDiff && node2->bAutoBuilt && !bMustNotAvg )
								{
									if ( Level->Model->FastLineCheck(AvgPos, node3->Location) )
									{
										if ( TestReach(AvgPos, node3->Location) ) 
											bFoundDiff = 0;
									}
									if ( bFoundDiff )
										bMustNotAvg = 1;
								}

								if ( bFoundDiff && Level->Model->FastLineCheck(node2->Location, node3->Location) )
								{
									if ( TestReach(node2->Location, node3->Location) ) 
										bFoundDiff = 0;
								}
								if ( bFoundDiff )
									break;
							}
						}
						if ( !bFoundDiff )
						{
							INT bBreakOut = 0;
							ANavigationPoint * Keeper;
							if ( node2->bAutoBuilt )
							{
								Keeper = node;
								debugf(TEXT("remove %s"),node2->GetName());
								Level->DestroyActor( node2 ); 
							}
							else
							{
								Keeper = node2;
								debugf(TEXT("remove %s"),node->GetName());
								Level->DestroyActor( node ); 
								bBreakOut = 1;
							}
							if ( !bMustNotAvg )
							{
								Keeper->Location = AvgPos;
								debugf(TEXT("Move %s to %f %f"),Keeper->GetName(), AvgPos.X, AvgPos.Y);
							}
							if ( bBreakOut )
								break;
						}
						Scout->SetCollisionSize(COMMONRADIUS, MINCOMMONHEIGHT);
					}
				}
			}
	}

	//Now add intermediate paths
	// if two nodes are walk reachable, then make sure they are less than 600 apart
	// or add an intermediate path
	for ( INT i=0; i<Level->Actors.Num(); i++ )
	{
		ANavigationPoint *node = (ANavigationPoint *)(Level->Actors(i)); 
		if (node && !node->bDeleteMe && node->IsA(ANavigationPoint::StaticClass()) && !node->IsA(ALiftCenter::StaticClass())  )
			for (INT j=0; j<Level->Actors.Num(); j++)
			{
				ANavigationPoint *node2 = (ANavigationPoint *)(Level->Actors(j)); 
				if ( ValidNode(node,node2)
						&& ((node->Location - node2->Location).SizeSquared() > 360000)
						&& (!node->bOneWayPath || (((node2->Location - node->Location) | node->Rotation.Vector()) > 0)) 
						&& Level->Model->FastLineCheck(node->Location, node2->Location) )
				{
					debugf(TEXT("Found potential distant pair %s (%f, %f) and %s (%f, %f)"),node->GetName(), node->Location.X, node->Location.Y, node2->GetName(), node2->Location.X, node2->Location.Y);
					// see if scout can walk without jumping node to node2
					Level->FarMoveActor(Scout, node->Location);
					Scout->Physics = PHYS_Walking;
					if ( Scout->pointReachable(node2->Location) ) 
					{
						// see if there is an intermediate path
						INT bFoundPath = 0;
						FLOAT TotalDist = (node->Location - node2->Location).Size();
						FLOAT TotalDistSq = TotalDist * TotalDist;

						for (INT k=0; k<Level->Actors.Num(); k++)
						{
							ANavigationPoint *node3 = (ANavigationPoint *)(Level->Actors(k)); 
							if (	ValidNode(node,node3) && (node3 != node2) 
									&& ((node->Location - node3->Location).SizeSquared() < TotalDistSq)
									&& ((node2->Location - node3->Location).SizeSquared() < TotalDistSq)
									&& (!node3->bOneWayPath || (((node2->Location - node3->Location) | node3->Rotation.Vector()) > 0)) 
									&& Level->Model->FastLineCheck(node->Location, node3->Location)
									&& Level->Model->FastLineCheck(node2->Location, node3->Location) )
							{
								FLOAT Dist13 = (node->Location - node3->Location).Size();
								FLOAT Dist32 = (node3->Location - node2->Location).Size();
							
								debugf(TEXT("Try %s Total %f versus %f + %f"),node3->GetName(),TotalDist, Dist13, Dist32);
								if ( Dist13 + Dist32 < 1.3 * TotalDist )
								{
									Level->FarMoveActor(Scout, node->Location);
									Scout->Physics = PHYS_Walking;
									if ( Scout->pointReachable(node3->Location) ) 
									{
										Level->FarMoveActor(Scout, node3->Location);
										Scout->Physics = PHYS_Walking;
										if ( Scout->pointReachable(node2->Location) )
										{
											debugf(TEXT("Found %s as intermediate"),node3->GetName());
											bFoundPath = 1;
											break;
										}
									}
								}
							}
						}
						// if not add a path
						if ( !bFoundPath && Level->FarMoveActor(Scout, 0.5 * (node->Location + node2->Location)) )
							newPath(Scout->Location);
					}
				}
			}
	}
	//***************************
	int numpaths = 0;
	for (INT i=0; i<Level->Actors.Num(); i++)
	{
		AActor *Actor = Level->Actors(i);
		if (Actor )
		{
			if ( Actor->IsA(APawn::StaticClass()) )
				Actor->SetCollision(1, 1, 1); //turn Pawn collision back on
			else if ( Actor->IsA(ANavigationPoint::StaticClass()) 
					&& ((ANavigationPoint *)Actor)->bAutoBuilt )
				numpaths++;
		}
	}

	return numpaths; 
	unguard;
}


//newPath() 
//- add new pathnode to level at position spot
ANavigationPoint* FPathBuilder::newPath(FVector spot)
{
	guard(FPathBuilder::newPath);
	
	if (Scout->CollisionHeight < 48) // fixme - base on Skaarj final height
		spot.Z = spot.Z + 48 - Scout->CollisionHeight;
	UClass *pathClass = FindObjectChecked<UClass>( ANY_PACKAGE, TEXT("PathNode") );
	APathNode *addedPath = (APathNode *)Level->SpawnActor( pathClass, NAME_None, NULL, NULL, spot,FRotator(0,0,0),NULL,1 );
	if ( !addedPath )
	{
		debugf(TEXT("Failed to add path!"));
		return NULL;
	}
	debugf(TEXT("Added new path %s at %f %f"),addedPath->GetName(), addedPath->Location.X, addedPath->Location.Y);
	addedPath->bAutoBuilt = 1;
	//clear pathnode reachspec lists
	for (INT i=0; i<16; i++)
	{
		addedPath->Paths[i] = -1;
		addedPath->upstreamPaths[i] = -1;
	}

	return addedPath;
	unguard;
};


/*getScout()
Find the scout actor in the level. If none exists, add one.
*/ 

void FPathBuilder::getScout()
{
	guard(FPathBuilder::getScout);
	Scout = NULL;
	for( INT i=0; i<Level->Actors.Num(); i++ )
	{
		AActor *Actor = Level->Actors(i); 
		if (Actor && Actor->IsA(AScout::StaticClass()))
			Scout = (APawn *)Actor;
	}
	if( !Scout )
	{
		UClass *scoutClass = FindObjectChecked<UClass>( ANY_PACKAGE, TEXT("Scout") );
		Scout = (APawn *)Level->SpawnActor( scoutClass );
	}
	Scout->SetCollision(1,1,1);
	Scout->bCollideWorld = 1;
	Level->SetActorZone( Scout,1,1 );
	return;
	unguard;
}


int FPathBuilder::findScoutStart(FVector start)
{
	guard(FPathBuilder::findScoutStart);
	
	if (Level->FarMoveActor(Scout, start)) //move Scout to starting point
	{
		//slide down to floor
		FCheckResult Hit(1.f);
		FVector Down = FVector(0,0, -50);
		Hit.Normal.Z = 0.f;
		INT iters = 0;
		while (Hit.Normal.Z < 0.7f)
		{
			Level->MoveActor(Scout, Down, Scout->Rotation, Hit, 1,1);
			if ((Hit.Time < 1.f) && (Hit.Normal.Z < 0.7f)) 
			{
				//adjust and try again
				FVector OldHitNormal = Hit.Normal;
				FVector Delta = (Down - Hit.Normal * (Down | Hit.Normal)) * (1.f - Hit.Time);
				if( (Delta | Down) >= 0 )
				{
					Level->MoveActor(Scout, Delta, Scout->Rotation, Hit, 1,1);
					if ((Hit.Time < 1.f) && (Hit.Normal.Z < 0.7f))
					{
						FVector downDir = Down.SafeNormal();
						Scout->TwoWallAdjust(downDir, Delta, Hit.Normal, OldHitNormal, Hit.Time);
						Level->MoveActor(Scout, Delta, Scout->Rotation, Hit, 1,1);
					}
				}
			}
			iters++;
			if (iters >= 50)
			{
				debugf(NAME_DevPath,TEXT("No valid start found"));
				return 0;
			}
		}
		//debugf(NAME_DevPath,"scout placed on valid floor");
		return 1;
 	}

	debugf(NAME_DevPath,TEXT("Scout didn't fit"));
	return 0;
	unguard;
}

void FPathBuilder::testPathsFrom(FVector start)
{
	guard(FPathBuilder::testPathsFrom);
	
	if ( (!findScoutStart(start) 
		|| (Abs(Scout->Location.Z - start.Z) > Scout->CollisionHeight))
		&& !findScoutStart(start + FVector(0,0,20)) )
		return;

	// do a walk from this start, following wall.
	// make sure there is always a valid path anchor
	FVector StartDir(1,0,0);
	FLOAT TurnDir = 1;
	for ( FLOAT f=MAXCOMMONRADIUS; f>=MINCOMMONRADIUS; f-=7.f )
	{
		Scout->SetCollisionSize(f, MINCOMMONHEIGHT);
		Pass2From(start, StartDir, TurnDir);
		TurnDir *= -1;
		FLOAT Temp = StartDir.X;
		StartDir.X = StartDir.Y;
		StartDir.Y = Temp;
	}

	unguard;
}

void FPathBuilder::Pass2From(FVector start, FVector moveDirection, FLOAT TurnDir)
{
	guard(FPathBuilder::Pass2From);

	debugf(TEXT("WALK WITH COLLISION SIZE %f %f"),Scout->CollisionRadius, Scout->CollisionHeight);
	for (INT i=0; i<Level->Actors.Num(); i++) 
	{
		ANavigationPoint *Nav = (ANavigationPoint *)(Level->Actors(i));
		if ( Nav && !Nav->bDeleteMe && Nav->IsA(ANavigationPoint::StaticClass()) )
		{
			Nav->visitedWeight = 1;
			Nav->bEndPoint = 0;
		}
	}

	// find a wall
	INT stillmoving = 1;
	FCheckResult Hit(1.f);
	while (stillmoving == 1)
		stillmoving = TestWalk(moveDirection * 16.f, Hit, 4.1f, 0); 

	FVector BlockNormal = -1 * moveDirection;
	FindBlockingNormal(BlockNormal);
	moveDirection = FVector(BlockNormal.Y, -1 * BlockNormal.X, 0); 

	// find an anchor
	ANavigationPoint *Anchor = NULL;
	FLOAT BestDistSq = 640000;
	for ( INT i=0; i<Level->Actors.Num(); i++ )
	{
		ANavigationPoint *Nav = (ANavigationPoint *)(Level->Actors(i));
		if ( Nav && !Nav->bDeleteMe && Nav->IsA(ANavigationPoint::StaticClass()) && ((Nav->Location - Scout->Location).SizeSquared() < BestDistSq) )
		{
			FVector RealLoc = Scout->Location;
			if ( TestReach(Nav->Location,RealLoc) )
			{
				debugf(TEXT("----------------------Anchor %s"), Nav->GetName());
				Anchor = Nav;
				Anchor->Velocity = moveDirection;
				Anchor->bEndPoint = 1;
				BestDistSq = (Anchor->Location - Scout->Location).SizeSquared();
			}
			Level->FarMoveActor(Scout, RealLoc);
		}
	}

	// follow wall - if turn right, look for new anchor, 
	// if turn left, look for new anchor, or keep checking old
	// if I find two anchors which are marked in a row (bEndPoint == 1), I'm done
	INT LoopComplete = 0;
	INT NumSteps = 0;
	INT TotalSteps = 0;
	FVector TestLoc = Scout->Location;
	stillmoving = 1;
	INT RightTurns = 0;

	while ( (LoopComplete <2) && (NumSteps < 1000) )
	{
		if ( (TestLoc - Scout->Location).SizeSquared() > 40000 )
		{
			NumSteps = 0;
			TestLoc = Scout->Location;
		}
		else
			NumSteps++;
		TotalSteps++;
		if ( TotalSteps > 200 )
		{
			debugf(TEXT("Total steps out of bounds"));
			NumSteps = 2000;
		}
		FVector OldPos = Scout->Location;
		// try right turn
		INT tryturn = TestWalk(FVector(-1 * TurnDir * moveDirection.Y, TurnDir * moveDirection.X, 0) * 16, Hit, 12, 0);
		if ( tryturn == 1 )
			stillmoving = 1;
		else
			stillmoving = TestWalk(moveDirection * 16.f, Hit, 4.1f, 0); 
		//debugf(TEXT("Now at %f %f"),Scout->Location.X, Scout->Location.Y);

		// check good floor
		Level->SingleLineCheck(Hit, Scout, Scout->Location - FVector(0,0,(Scout->CollisionHeight + Scout->MaxStepHeight + 4.f)) , Scout->Location, TRACE_VisBlocking, FVector(16,16,1));

		if( Anchor && Hit.Time<1.f )
		{
			FVector RealLoc = Scout->Location;
			if ( !TestReach(Anchor->Location,RealLoc) )
			{
				// find new anchor or place one
				//debugf(TEXT("ANCHOR NO LONGER VALID"));
				ANavigationPoint *OldAnchor = Anchor;
				ANavigationPoint *ClosestPath = NULL;
				FLOAT ClosestDistSq = 65536.f;
				BestDistSq = 65536.f;
				for (INT i=0; i<Level->Actors.Num(); i++) 
				{
					ANavigationPoint *Nav = (ANavigationPoint *)(Level->Actors(i));
					if ( Nav && !Nav->bDeleteMe && Nav->IsA(ANavigationPoint::StaticClass()) )
					{
						FLOAT NewDistSq = (Nav->Location - Scout->Location).SizeSquared();

						if ( NewDistSq < BestDistSq )
						{
							//debugf(TEXT("Try %s as anchor"),Nav->GetName());
							if ( TestReach(Nav->Location,RealLoc) )
							{
								//if ( OldAnchor )
								//	debugf(TEXT("Can reach it - try from %s"), OldAnchor->GetName());
								if ( NewDistSq < ClosestDistSq )
								{
									ClosestDistSq = NewDistSq;
									ClosestPath = Nav;
								}
								if ( TestReach(OldAnchor->Location, Nav->Location) )
								{
									//debugf(TEXT("Reached new anchor again"));
									if ( TestReach(OldPos,Nav->Location) )
									{
										debugf(TEXT("---------------------- New Anchor %s"), Nav->GetName());
										Anchor = Nav;
										if ( Anchor->bEndPoint && ((Anchor->Velocity | moveDirection) > 0.9) )
											LoopComplete++;
										else
											LoopComplete = 0;
										Anchor->Velocity = moveDirection;
										Anchor->bEndPoint = 1;
										
										Level->FarMoveActor(Scout, RealLoc);
										BestDistSq = (Anchor->Location - Scout->Location).SizeSquared();
									}
								}
							}
							Level->FarMoveActor(Scout, RealLoc);
						}
					}
				}

				if ( Anchor == OldAnchor )
				{
					INT bAdjusted = 0;
					debugf(TEXT("didn't find new anchor"));
					if ( ClosestDistSq < 2500.f )
					{
						INT bNew = 1;
						INT bAvg = 1;
						FVector AvgPos = 0.5 * (OldPos + ClosestPath->Location);
						//look to see if can reach same paths as old spot in new
						debugf(TEXT("Check closest path %s"),ClosestPath->GetName());
						Level->FarMoveActor(Scout, ClosestPath->Location);
						for (INT i=0; i<Level->Actors.Num(); i++) 
						{
							ANavigationPoint *Nav = (ANavigationPoint *)(Level->Actors(i));
							if ( ValidNode(ClosestPath, Nav)
								&& ((ClosestPath->Location - Nav->Location).SizeSquared() < 640000) )
							{
								Scout->Physics = PHYS_Walking;
								if ( Scout->pointReachable(Nav->Location) )
								{
									if ( bNew )
									{
										bNew = TestReach(OldPos,Nav->Location);
										Level->FarMoveActor(Scout, ClosestPath->Location);
									}
									if ( bAvg )
									{
										bAvg = TestReach(AvgPos,Nav->Location);
										Level->FarMoveActor(Scout, ClosestPath->Location);
									}
									if ( !bNew && !bAvg )
										break;
								}
							}
						}

						if ( bAvg )
						{
							Anchor = ClosestPath;
							Anchor->Location = AvgPos;
							bAdjusted = 1;
						}
						else if ( bNew )
						{
							Anchor = ClosestPath;
							Anchor->Location = OldPos;
							bAdjusted = 1;
						}
						else if ( ClosestDistSq < 1000.f )
							bAdjusted = 1;
					}

					if ( !bAdjusted )
					{
						Anchor = newPath(OldPos);
						if ( !Anchor )
							return;

						if ( (OldAnchor->Location - Anchor->Location).SizeSquared() < 1000.f )
						{
							Level->DestroyActor( Anchor ); 
							Anchor = OldAnchor;
						}
						else if ( ClosestPath && ((ClosestPath->Location - Anchor->Location).SizeSquared() < 1000.f) )
						{
							Level->DestroyActor( Anchor ); 
							Anchor = ClosestPath;
						}
						else
						{
							for ( INT i=0; i<Level->Actors.Num(); i++ )
							{
								AActor *Actor = Level->Actors(i);
								if ( Actor && (Actor != Anchor) && !Actor->bDeleteMe && Actor->IsA(ANavigationPoint::StaticClass()) )
								{
									ANavigationPoint *Nav = (ANavigationPoint *)Actor;
									if ( ((Anchor->Location - Nav->Location).SizeSquared() < 1000.f)
										&& Level->Model->FastLineCheck(Anchor->Location, Nav->Location) )
									{
										Level->DestroyActor( Anchor ); 
										Anchor = Nav;
										break;
									}
								}
							}
						}
						Anchor->visitedWeight++;
						if ( Anchor->visitedWeight > 10 )
							NumSteps = 5000;
						Anchor->Velocity = moveDirection;
						Anchor->bEndPoint = 1;
						debugf(TEXT("---------------------- ADD Anchor %s at %f %f"), Anchor->GetName(), Anchor->Location.X, Anchor->Location.Y);
					}

					if ( Anchor != OldAnchor )
						TotalSteps = 0;
				}
			}
			Level->FarMoveActor(Scout, RealLoc);
		}
		if ( tryturn == 1 )
		{
			if ( !Anchor && (RightTurns == 0) )
				Anchor = newPath(OldPos);

			RightTurns++;
			if ( RightTurns > 100 )
				 NumSteps = 3000;
			if ( RightTurns < 3)
				moveDirection = FVector(-1 * TurnDir * moveDirection.Y, TurnDir * moveDirection.X, 0);
			//debugf(TEXT("turn right dir %f %f"), moveDirection.X, moveDirection.Y);
		}
		else if ( stillmoving != 1 )
		{
			RightTurns = 4;
			BlockNormal = -1 * moveDirection;
			FindBlockingNormal(BlockNormal);
			moveDirection = FVector(-1 * TurnDir * BlockNormal.Y, TurnDir * BlockNormal.X, 0); 
			//debugf(TEXT("turn left dir %f %f"), moveDirection.X, moveDirection.Y);
		}
		else RightTurns = 0;
	}
	debugf(TEXT("Num steps %d"),NumSteps);

	unguard;
}

INT FPathBuilder::ValidNode(ANavigationPoint *node, AActor *node2)
{
	guard(FPathBuilder::TestReach);
	return (node2 && (node2 != node) && !node2->bDeleteMe 
		&& node2->IsA(ANavigationPoint::StaticClass()) 
		&& !node2->IsA(ALiftCenter::StaticClass()) );
	unguard;
}

INT FPathBuilder::TestReach(FVector Start, FVector End)
{
	guard(FPathBuilder::TestReach);

	FVector OldLoc = Scout->Location;
	Level->FarMoveActor(Scout, Start);
	Scout->Physics = PHYS_Walking;
	INT bResult = Scout->pointReachable(End);
	Level->FarMoveActor(Scout, OldLoc,0,1);
	return bResult;
	unguard;
}
	

INT FPathBuilder::TestWalk(FVector WalkDir, FCheckResult Hit, FLOAT Threshold, INT bAdjust)
{
	guard(FPathBuilder::TestWalk);

	FVector OldLoc = Scout->Location;
	INT Result = Scout->walkMove(WalkDir, Hit, NULL, Threshold, bAdjust);

	if ( Result == 1 )
	{
		Level->SingleLineCheck(Hit, Scout, Scout->Location - FVector(0,0,(Scout->CollisionHeight + Scout->MaxStepHeight + 4.f)) , Scout->Location, TRACE_VisBlocking, FVector(16,16,1));
		if( Hit.Time < 1.f )
			return Result;
		Level->FarMoveActor(Scout,OldLoc,0,1);
		return -1;
	}

	return Result;
	unguard;
}

void FPathBuilder::FindBlockingNormal(FVector &BlockNormal)
{
	guard(FPathBuilder::FindBlockingNormal);

	FCheckResult Hit(1.f);
	Level->SingleLineCheck(Hit, Scout, Scout->Location - BlockNormal * 16, Scout->Location, TRACE_VisBlocking, Scout->GetCylinderExtent());
	if( Hit.Time < 1.f )
	{
		BlockNormal = Hit.Normal;
		return;
	}

	// find ledge
	FVector Destn = Scout->Location - BlockNormal * 16;
	FVector TestDown = FVector(0,0, -1 * Scout->MaxStepHeight);
	Level->SingleLineCheck(Hit, Scout, Destn + TestDown, Destn, TRACE_VisBlocking, Scout->GetCylinderExtent());
	if( Hit.Time < 1.f )
	{
		//debugf(TEXT("Found landing when looking for ledge"));
		return;
	}
	Level->SingleLineCheck(Hit, Scout, Scout->Location + TestDown, Destn + TestDown, TRACE_VisBlocking, Scout->GetCylinderExtent());

	if( Hit.Time < 1.f )
		BlockNormal = Hit.Normal;
	
	unguard;
}




