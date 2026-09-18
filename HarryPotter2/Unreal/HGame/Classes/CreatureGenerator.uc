
class CreatureGenerator extends HPawn;

const NUM_BASE_CHARS = 16;
const MAX_CREATURES  = 10;

var()	class<HPawn> 	BaseCreatureToSpawn[16];
var()	float			BaseCreatureGroundSpeed[16];
var()	float			BaseCreatureLife[16];
var()	name			BaseCreatureAnim[16];
var()	string			BaseBumpLineSet[16];

var()	bool			bCaptureHarry;

var()	int				TooFar;

var()	int				MinDelayInSeconds;
var()	int				MaxDelayInSeconds;

var()	int				MaxCreaturesOnFastMachine;
var()	int				MaxCreaturesOnSlowMachine;

var()	name 			FirstPatrolPoint_ObjectName;

var()	float			TriggerWaitingTime;

//*******************************************************************************

var		BaseCam			Camera;

var		PatrolPoint		FirstPP;

var		HPawn			Creatures[10];
var		float			TotalLifes[10];
var		float			Lifes[10];

var		int				HowManyBaseCreatures;
var		int				PreviousBaseCreatureIndex;

var		int				NumCreatures;
var		int				MaxCreatures;
var		float			fCurrTime;
var		float			fWaitTime;

var		bool			bGenerateCreature;

var		bool			bOff;

//*******************************************************************************
function PostBeginPlay()
{
	local BaseCam		cam;
	local PatrolPoint	pp;
	local int			i;

	Super.PostBeginPlay();

	// Find the camera
	foreach AllActors( class'BaseCam', cam )
	{
		Camera = cam;
	}

	FirstPP = none;
	foreach AllActors( class'PatrolPoint', pp )
	{
		if( pp.name == FirstPatrolPoint_ObjectName )
		{
			FirstPP = pp;
			break;
		}
	}

	HowManyBaseCreatures = 0;
	for( i = 0; i < NUM_BASE_CHARS; i++)
	{
		if( BaseCreatureToSpawn[i] == none )
			break;
	}

	HowManyBaseCreatures = i;

}

function Tick(float deltaTime)
{
	// create something every few seconds
	fCurrTime += DeltaTime;
	if(fCurrTime < fWaitTime)
		return;

	UpdateCreaturesLife(fWaitTime);

	fCurrTime = 0.0f;

	// if somebody, accidentaly set 'wrong' delays
	if(MinDelayInSeconds >= MaxDelayInSeconds)
		fWaitTime = MinDelayInSeconds;
	else
		fWaitTime = RandRange(MinDelayInSeconds, MaxDelayInSeconds);

	DestroyCreatures();
	GenerateCreature();
}

//************************************************************************************************************
// This function will be called by the engine when it is time to resolve what this actor does
// if it is inside the current GameState or not.
event OnResolveGameState()
{
	// If we are not in the current gamestate then become hidden and have no collision
	if( !bInCurrentGameState )
	{
		// We are NOT in the current state
		bHidden = true;
		SetCollision(false,false,false);

		bOff = true;
	}
}

function bool TooFarFromHarry(vector loc)
{
	local float dist;

	dist = VSize(loc - playerHarry.location);
	if( dist > TooFar)
		return true;

	return false;
}

function bool CameraCanSeeYou(vector loc)
{
	if(TooFarFromHarry(loc))
		return false;

	return Camera.CameraCanSeeYou(loc);
}

function UpdateCreatureLife(int index, float deltaTime)
{
	if(Creatures[index] != none)
		Lifes[index] += deltaTime;
}

function UpdateCreaturesLife(float deltaTime)
{
	local int i;
	for( i = 0; i < MAX_CREATURES; i++ )
		UpdateCreatureLife(i, deltaTime);
}

function DestroyCreature(int index)
{
	local HPawn creature;

	creature = Creatures[index];
	if(Creature == none)
		return;

	if(Lifes[index]  < TotalLifes[index])
		return;

	if( CameraCanSeeYou(creature.location) )
		return;

	creature.Destroy();
	creature = none;

	NumCreatures--;
}

function DestroyCreatures()
{
	local int i;
	for( i = 0; i < MAX_CREATURES; i++ )
		DestroyCreature(i);
}

function GenerateCreature()
{
	local HPawn	a;
	local HChar	h;
	local int	i, j;
	local bool IsSoftRendering;

	IsSoftRendering = playerHarry.IsSoftwareRendering();

	if(	IsSoftRendering )
		MaxCreatures = MaxCreaturesOnSlowMachine;
	else
		MaxCreatures = MaxCreaturesOnFastMachine;

	// if was turn of, because of 'wrong' game state, do nothing
	if(bOff)
		return;

	// in case of trigger, see if we can generate creatures
	if( (TriggerWaitingTime != 0) && !bGenerateCreature )
		return;

	// too many creatures
	if(NumCreatures >= MaxCreatures)
		return;

	if(	FirstPP == none)
	{
		Log("CreatureGenerator: Couldn't find first patrol point");
		return;
	}

	// if we could see first patrol point, do nothing
	if( CameraCanSeeYou(FirstPP.location) )
		return;

	if(	HowManyBaseCreatures == 0 )
	{
		Log("CreatureGenerator: BaseCreatureToSpawn not set.");
		return;
	}

	// if we have just one base creature, select it
	if(	HowManyBaseCreatures == 1 )
		i = 0;

	// else select creature which is different from the previous one
	else
	{
		while(true)
		{
			i = Rand(HowManyBaseCreatures);
			if(i != PreviousBaseCreatureIndex)
			{
				PreviousBaseCreatureIndex = i;
				break;
			}
		}
	}

	a = spawn( BaseCreatureToSpawn[ i ], [SpawnLocation]FirstPP.Location, [SpawnRotation]rotator(FirstPP.NextPatrolPoint.Location - FirstPP.Location) );
	a.desiredrotation = a.rotation;

	if( a == none )
	{
		Log("CreatureGenerator: couldn't spawn the baseCreature:" $ BaseCreatureToSpawn[i]);
		return;
	}

	//Set him patrolling along the patrolpoints 
	a.ePatrolType = PATROLTYPE_PATROL_POINTS;
	a.firstPatrolPointObjectName = FirstPP.name;

	if( BaseCreatureGroundSpeed[i] != 0 )
		a.GroundSpeed = BaseCreatureGroundSpeed[i];

	// if animation was not set, or is not run animation, just play a walk one
	a.bPlayRunAnim = false;
	if(	BaseCreatureAnim[i] == 'run') 
		a.bPlayRunAnim = true;

	h = HChar(a);

	// set a bump lines
	h.bUseBumpLine		= true;
	h.bBumpCaptureHarry = bCaptureHarry;
	h.BumpLineSet		= BaseBumpLineSet[i];

	for( j = 0; j < MAX_CREATURES; j++ )
	{
		if(Creatures[j] == none)
		{
			Creatures[j]	= a;
			TotalLifes[j]	= BaseCreatureLife[i];
			Lifes[j]		= 0.0f;
			NumCreatures++;

			a.GotoState('patrol');
			return;
		}
	}
}

state() TriggerOpenTimed
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		GotoState( 'TriggerOpenTimed', 'Start' );
	}

Start:
	Disable( 'Trigger' );

	if(bGenerateCreature)
		bGenerateCreature = false;
	else
		bGenerateCreature = true;

	Sleep(TriggerWaitingTime);

	Enable( 'Trigger' );
}

//*****************************************************************************
defaultproperties
{
	bHidden=true;

	MaxCreaturesOnFastMachine=10
	MaxCreaturesOnSlowMachine=5

	TooFar=10000

	MinDelayInSeconds=10
	MaxDelayInSeconds=30

	PreviousBaseCreatureIndex=-1

	fCurrTime=0.000000
	fWaitTime=0.000000	// Eric wants to create first guy right away !!!

	InitialState=TriggerOpenTimed
	TriggerWaitingTime=0
	bGenerateCreature=false

	bOff=false

	bBlockActors=false
	bBlockPlayers=false

	bCaptureHarry=false

	NumCreatures=0
}