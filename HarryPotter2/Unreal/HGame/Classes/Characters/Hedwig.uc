// Class Name  : Hedwig
//
// Created on  : 04/19/2002
// Authored by : Janet Weddle
// 
// Description : Hedwig AI
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class Hedwig extends Characters;


// *** Variables

var vector vHome;

var() name HedwigPaths[5];
var()	int		shortestTimeBetweenSpells; 	// The minimum amount of time between actions
var()	int		longestTimeBetweenSpells;	// The maximum amount of time between actions
var name HedwigCurrentPath;

var()  bool    bDoNothingAtStartup;

// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


// --------------------------------------------------------------------------------------------
// *** Functions

function PreBeginPlay()
{
	Super.PreBeginPlay();
	
	// set our home location to be were we started
	vHome = location;

	// set Hedwig's path 
	HedwigCurrentPath = 'dummyPath';

	// have idle the default animation
	LoopAnim('breathe');
}

function OnEvent(name EventName)
{
	if( EventName == 'ActionDone' )
	{
		// Setting the timer here gaurantees that the timer won't fire when Peeves is moving from 
		// one path to another. This way he won't try to throw a spell or taunt Harry from inside a
		// wall. 
		SetMyTimer();

		// Continue on the same path
//		GotoSamePath();
	}	
}

// Set the timer
function SetMyTimer(optional float time)
{	
	local int minTimer;
	local int maxTimer;

	minTimer = shortestTimeBetweenSpells;
	maxTimer = longestTimeBetweenSpells;

	if ( time == 0 )
	{
		setTimer(minTimer + Rand(maxTimer-minTimer), false);
	}
	else
	{
		setTimer(time,false);
	}
}

// Change the IPSpeed to a low number. This will give the impression that the actor isn't moving
// (he is, just VERY slowly)
function StopOnSpline()
{
	IPSpeed = 0.000001;
}


// Change the IPSpeed to 0. This will set the speed to the editor default
function ContinueOnSpline()
{
	IPSpeed = 0;
}

function GotoNewPath()
{
	local int select;
	local int count, numPaths;
	local name tempName, singlePath;

	// Check the number of paths
	for ( count=0; count<5; count++ )
	{
		tempName = HedwigPaths[count];
		if ( tempName != 'none' )
		{
			numPaths++;
			singlePath = tempName;
		}
	}

	switch (numPaths)
	{
	case 0:
		log('You must enter a pathname for Hedwig');

		// The default path name when entering interpolation points into the editor is 'Interpolation'
		//HedwigCurrentPath = 'Interpolation'; 
		break;
	case 1:
		// There is only one path available
		HedwigCurrentPath = singlePath;
		break;
	default:

		select = rand(5);

		while ( HedwigPaths[select] == HedwigCurrentPath )
		{
			select = rand(5);
		}

		HedwigCurrentPath = HedwigPaths[select];
		break;
	}


	// Set Hedwig on the spline
	FollowSplinePath( HedwigCurrentPath ,	//optional name  PathTagName
					SplineSpeed ,					//optional float speed
					0 ,						//optional float accel
					,						//optional name  StartPointName
					 						//optional name  EndPointName
					);

}

function PlayerCutCapture()
{
	gotoState('CutIdle');
}

function PlayerCutRelease()
{
	gotoState('stateIdle');
}


// --------------------------------------------------------------------------------------------
// *** States
auto state stateIdle
{
	begin:
	if( !bDoNothingAtStartup )
	{
		if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": auto stateIdle" );

		LoopAnim('breathe');

		sleep(2.0);

		LoopAnim('TakeOff');

		GotoNewPath();
	}
}


function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);
}


state CutIdle
{

begin:

	// Get rid of the spline so Hedwig can 'fly free'
	DestroyControllers();

	Acceleration = vect(0,0,0);
	Velocity =     vect(0,0,0);

}


// This state is from HPawn and is needed so I can check stuff while Hedwig is on the spline.
// Otherwise I can't test anything. 
state patrolFollowSpline
{

	function EndState()
	{
		Super.EndState();

		setPhysics(PHYS_FLYING);
		SetMyTimer(0.0);

	}

	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		// What direction is Hedwig flying next?

	}


	function Timer()
	{
		SetMyTimer(3.0);
	}



	begin:

	// Can't do anything here. 

}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skowlbarnMesh'
	DrawType=DT_Mesh
	Menuname="Hedwig"
	Physics=PHYS_FLYING

	SightRadius=4000
	PeripheralVision=0

	RotationRate=(Pitch=100000,Yaw=100000,Roll=100000)

	shortestTimeBetweenSpells=5;
	longestTimeBetweenSpells=30;
	bDoEyeBlinks=false
}