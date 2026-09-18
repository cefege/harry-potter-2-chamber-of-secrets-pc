class LifecycleGateActor extends Actor;

var int SpawnedCount;
var int SetInitialStateCount;
var int PreBeginPlayCount;
var int BeginPlayCount;
var int PostBeginPlayCount;
var int BeginStateCount;
var int EndStateCount;
var int IteratorYieldCount;
var int DestroyedCount;
var int StateStage;
var Actor Seen;

event Spawned()
{
	SpawnedCount++;
}
event PreBeginPlay()
{
	PreBeginPlayCount++;
	Super.PreBeginPlay();
}

event BeginPlay()
{
	BeginPlayCount++;
	Super.BeginPlay();
}

event PostBeginPlay()
{
	PostBeginPlayCount++;
	Super.PostBeginPlay();
}


event SetInitialState()
{
	SetInitialStateCount++;
	Super.SetInitialState();
}

event Destroyed()
{
	DestroyedCount++;
}

auto state Scan
{
	function BeginState()
	{
		BeginStateCount++;
	}

	function EndState()
	{
		EndStateCount++;
	}

Begin:
	StateStage = 10;
	foreach AllActors(class'Actor', Seen, 'LifecycleGateProbe')
		IteratorYieldCount++;
	Seen = None;
	GotoState('AwaitResume');
}

state AwaitResume
{
	function BeginState()
	{
		BeginStateCount++;
	}

	function EndState()
	{
		EndStateCount++;
	}

Begin:
	StateStage = 20;
	Sleep(0.01);
	StateStage = 30;
	Destroy();
}

defaultproperties
{
	bGameRelevant=True
	bHidden=True
	bCollideActors=False
	bBlockActors=False
	bBlockPlayers=False
}
