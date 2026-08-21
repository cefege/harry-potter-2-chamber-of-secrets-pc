class SleepingGoyle extends Characters;

/*
Animations:

Goyle_Sleeping
Goyle_Groggy
Goyle_WakeUP
Goyle_SitUp_Idle

lyingDown	???
getUp		???

*/

var		int			WakeLevel;

var		TriggerChangeLevel TriggerNextLevel;

var()	int		WakeUpDistance;
var()	string	CurrLevelName;
var()	string	NextLevelName;

function PostBeginPlay()
{
	local ChickenLeg leg;

	Super.PostBeginPlay();

	TriggerNextLevel = spawn(class'TriggerChangeLevel', , , , );
	if(TriggerNextLevel != none )
	{
		TriggerNextLevel.NewMapName = NextLevelName;
		TriggerNextLevel.SetCollision(false, false, false);
	}

	leg = spawn(class'ChickenLeg', , , , );
	if( leg != none )
	{
		leg.SetOwner(self);
		leg.AttachToOwner('RightHand');		//Bip01 R Forearm
	}
}

function Tick(float deltaT)
{
	super.Tick(deltaT);

	if(playerHarry.bFinishPickBitOfGoyle)
	{
		playerHarry.bFinishPickBitOfGoyle = false;

		// go to the next level
		if(TriggerNextLevel != none)
			TriggerNextLevel.ProcessTrigger();
	}
}

function PigWakeGoyle()
{
	// if he is not sleeping, he could not wake up
	if( GetStateName() != 'stateSleep')
		return;

	// if Harry started to pickup bit of Goyle, do not wake up
	if(playerHarry.GetStateName() == 'statePickBitOfGoyle')
		return;

	WakeLevel++;
	gotostate('stateWakeUp');
}

function bool TooCloseToHarry()
{
 	local vector vDifference;
	vDifference	 = playerHarry.location - location;

	if( vsize(vDifference) <= WakeUpDistance )
		return true;

	return false;
}

auto state stateSleep
{
	function bump( actor other )
	{
		if( other == playerHarry )
		{
			Harry(other).gotostate('statePickBitOfGoyle');
		}
	}

	begin:

	PlayAnim('Goyle_Sleeping');
	FinishAnim();
	Sleep(0.01);	  // just in case

	Goto 'begin';
}

state stateBustHarry
{
	begin:

	PlayAnim('Goyle_SitUp_Idle');
	FinishAnim();

	// Always load "save0.usa".
	PlayerHarry.ConsoleCommand("LoadGame 0");
}

state stateWakeUp
{
	begin:

	if(WakeLevel == 1)
	{
		PlayAnim('Goyle_Groggy');
		FinishAnim();
	}
	else if(WakeLevel == 2)
	{
		PlayAnim('Goyle_Groggy');
		FinishAnim();
	}
	else if(WakeLevel == 3)
	{
		PlayAnim('Goyle_Groggy');
		FinishAnim();
	}
	else if(WakeLevel == 4)
	{
		PlayAnim('Goyle_WakeUp');
		FinishAnim();
		gotostate('stateBustHarry');
	}

	if(!TooCloseToHarry())
		gotostate('stateSleep');
	else
	{
		WakeLevel++;
		gotostate('stateWakeUp');
	}
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skGoyleMesh'

	CollisionRadius=15
	CollisionHeight=44

	WakeUpDistance=200

	CurrLevelName="Adv6Goyle"
	NextLevelName="Adv7SlythComRoom"

	bDoEyeBlinks=false
}
 