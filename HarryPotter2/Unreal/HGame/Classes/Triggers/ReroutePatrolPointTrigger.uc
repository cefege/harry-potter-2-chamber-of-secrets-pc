
class ReroutePatrolPointTrigger extends trigger;

var() name SourcePatrolPoint_Tag; //PatrolPoint to modify, by tag
var() name SourcePatrolPoint_ObjectName; //Same as SourcePatrolPoint_Tag, but you can use the name of the object instead of the Tag

var() name DestPatrolPoint_Tag; //Where the source PatrolPoint should now link to, by tag
var() name DestPatrolPoint_ObjectName;


//*******************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	ProcessTrigger();
}

function touch(actor other)
{
	ProcessTrigger();
}

//*******************************************************************************
function ProcessTrigger()
{
	local PatrolPoint   sp, dp;

	//Find Source patrolpoint
	if( SourcePatrolPoint_Tag != '' )
	{
		foreach AllActors( class'PatrolPoint', sp, SourcePatrolPoint_Tag )
			break;
	}
	else
	{
		foreach AllActors( class'PatrolPoint', sp )
			if( sp.name == SourcePatrolPoint_ObjectName )
				break;
	}

	//Find Dest patrolpoint
	if( DestPatrolPoint_Tag != '' )
	{
		foreach AllActors( class'PatrolPoint', dp, DestPatrolPoint_Tag )
			break;
	}
	else
	{
		foreach AllActors( class'PatrolPoint', dp )
			if( dp.name == DestPatrolPoint_ObjectName )
				break;
	}

	if( sp == none )
	{
		Log("ReroutePatrolPointTrigger: Couldn't find Source patrol point");
		return;
	}

	if( dp == none )
	{
		Log("ReroutePatrolPointTrigger: Couldn't find Dest patrol point");
		return;
	}
Log("************* reroute "$sp$" to "$dp);
	sp.NextPatrolPoint = dp;
}


//*****************************************************************************
defaultproperties
{

}