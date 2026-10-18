/*=============================================================================
	UnReach.cpp: Reachspec creation and management

	These methods are members of the FReachSpec class, 

	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Steven Polge 3/97
=============================================================================*/
#include "EnginePrivate.h"
#include "HP2EngineServices.h"

/* Monster path 
 returns true if supports typical (Skaarj) movement requirements
 */
int FReachSpec::MonsterPath()
{
	guard(FReachSpec::MonsterPath);

	return hp2_rs_engine_reach_monster_path( this );
	unguard;
}

int FReachSpec::BotOnlyPath()
{
	guard(FReachSpec::BotOnlyPath);

	return hp2_rs_engine_reach_bot_only_path( this );
	
	unguard;
}
/* 
+ adds two reachspecs - returning the combined reachability requirements and total distance 
Note that Start and End are not set
*/
FReachSpec FReachSpec::operator+ (const FReachSpec &Spec) const
{
	guard(FReachSpec::operator+);
	FReachSpec Combined;
	
	hp2_rs_engine_reach_combine( this, &Spec, &Combined );
	
	return Combined; 
	unguard;
}
/* operator <=
Used for comparing reachspecs reach requirements
less than means that this has easier reach requirements (equal or easier in all categories,
does not compare distance, start, and end
*/
int FReachSpec::operator<= (const FReachSpec &Spec)
{
	guard(FReachSpec::operator<=);
	int result = hp2_rs_engine_reach_less_equal( this, &Spec );
	return result; 
	unguard;
}

/* operator ==
Used for comparing reachspecs for choosing the best one
does not compare start and end
*/
int FReachSpec::operator== (const FReachSpec &Spec)
{
	guard(FReachSpec::operator ==);
	int result = hp2_rs_engine_reach_equal( this, &Spec );
	
	return result; 
	unguard;
}

/* defineFor()
initialize the reachspec for a  traversal from start actor to end actor.
Note - this must be a direct traversal (no routing).
Returns 1 if the definition was successful (there is such a reachspec), and zero
if no definition was possible
*/

int FReachSpec::defineFor(AActor * begin, AActor * dest, APawn * Scout)
{
	guard(FReachSpec::defineFor);
	return hp2_rs_engine_reach_define_for( &GHp2EngineServices, this, begin, dest, Scout );
	unguard;
}

int FReachSpec::findBestReachable(FVector &begin, FVector &Destination, APawn * Scout)
{
	guard(FReachSpec::findBestReachable);

	return hp2_rs_engine_reach_find_best( &GHp2EngineServices, this, &begin, &Destination, Scout );
	unguard;
}



