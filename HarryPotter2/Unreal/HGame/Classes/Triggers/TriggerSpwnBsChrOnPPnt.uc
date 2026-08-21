
class TriggerSpwnBsChrOnPPnt extends trigger;

const NUM_BASE_CHARS = 8;
var() class<HPawn>     BaseCharToSpawn[8];
var() float            BaseCharGroundSpeed[8];

var() name StartPatrolPoint_Tag; //PatrolPoint to basechar on, by tag
var() name StartPatrolPoint_ObjectName; //Same as StartPatrolPoint_Tag, but you can use the name of the object instead of the Tag



//*******************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	ProcessTrigger();
}

function touch(actor other)
{
	super.touch(other);

	ProcessTrigger();
}

//*******************************************************************************
function ProcessTrigger()
{
	local PatrolPoint   sp;
	local HPawn         a;
	local int           n, i;

	//Find Source patrolpoint
	if( StartPatrolPoint_Tag != '' )
	{
		foreach AllActors( class'PatrolPoint', sp, StartPatrolPoint_Tag )
			break;
	}
	else
	{
		foreach AllActors( class'PatrolPoint', sp )
			if( sp.name == StartPatrolPoint_ObjectName )
				break;
	}


	if( sp == none )
	{
		Log("TriggerSpwnBsChrOnPPnt: Couldn't find Source patrol point");
		return;
	}

	//Cound the array
	for( n = 0; n < NUM_BASE_CHARS; n++)
	{
		if( BaseCharToSpawn[n] == none )
			break;
	}

	if( n == 0 )//BaseCharToSpawn == none )
	{
		Log("TriggerSpwnBsChrOnPPnt: BaseCharToSpawn not set to a baseChar");
		return;
	}

	i = Rand(n);

	a = spawn( BaseCharToSpawn[ i ], [SpawnLocation]sp.Location, [SpawnRotation]rotator(sp.NextPatrolPoint.Location - sp.Location) );

	if( a == none )
	{
		Log("TriggerSpwnBsChrOnPPnt: couldn't spawn the baseChar:" $ BaseCharToSpawn[i]);
		return;
	}

	//Set him patrolling along the patrolpoints
	//a.bFollowPatrolPoints = true;
	a.ePatrolType = PATROLTYPE_PATROL_POINTS;
	a.firstPatrolPointObjectName = sp.name;

	if( BaseCharGroundSpeed[i] != 0 )
		a.GroundSpeed = BaseCharGroundSpeed[i];

	a.GotoState('patrol');
}


//*****************************************************************************
defaultproperties
{

}