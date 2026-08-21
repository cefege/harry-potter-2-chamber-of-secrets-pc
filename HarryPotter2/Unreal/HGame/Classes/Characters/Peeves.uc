
// Class Name  : Peeves
//
// Created on  : 04/9/2002
// Authored by : Janet Weddle
// 
// Description : Peeves Generic AI
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class Peeves extends baseBoss;


// *** Variables

var vector	vHome;	

var	float	fParticleTrailLife;			// Length of time the particle trail exists	
var ParticleFX ParticleFXActor;			// Particle fx

var sound peevesVoice;					// The sound of Peeves
var string  sSoundID;					// The name of the sound file

var vector		vTargetDir;				// stores our target direction (normalized)
var int			spellCount, randSpells;
var float		sleepTime;
var BossEncounterTrigger peevesTrigger;
var() float HitDamage;
var bool bGameOver;
var vector vDir;
var() name DeathPatrolPoint;	// Annoying Peeves needs a place to run to so he can be destroyed.

enum PeevesType
{
	Annoyance,
	MiniBoss,
};

var() PeevesType type;

var sound throwSound;

var PatrolPoint currentPoint;

// *** Constants

const		BOOL_DEBUG_AI	= false;  // if true this will spit out debug AI info


// --------------------------------------------------------------------------------------------
// *** Functions

function PreBeginPlay()
{
	Super.PreBeginPlay();
	
	bFlipPushable	= true;
	lockSpell		= true;

	// This is where he starts out
	vHome = location;

	SetCollision( true, false, false );

}


function PostBeginPlay()
{
	Super.PostBeginPlay();

	if ( type == Annoyance )
	{
		DeathPatrolPoint='DeathPatrolPoint';

		// Peeves is faster when he's an annoyance
		airSpeed = 190;

		FindClosestDeathPoint();
	}
	else
	{
		GotoFirstPoint();
		
		foreach AllActors( class'BossEncounterTrigger', peevesTrigger, 'BossPeevesTrigger' )
			break;
	}

	ParticleFXActor = spawn(class'GhostTrail',,,Location);
	ParticleFXActor.Lifetime.Base = fParticleTrailLife;

}

static function vector GetFacing( actor A )
{
	return vec(1,0,0) >> A.Rotation;
}


function bool HandleSpellSkurge( optional baseSpell spell, optional vector vHitLocation )
{	
	if ( type == Annoyance )
	{
		gotoState('DontAnnoyHarry');
	}
	else
	{
		gotoState('stateHitBySpell');
	}

	return true;
	
}

// The function used by the enemyHealthBar for the boss battle. A return value of 0 will result in the
// HealthBar disappearing.
function float GetHealth()
{
	return float(Health) / 100;
}

function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	ParticleFXActor.SetLocation( Location+vect(0,0,-15) );
}

function PeevesOuch()
{
	switch( Rand(3) )
	{
	case 0: 
		sSoundID = "peeves_ow01";
		break;
	case 1: 
		sSoundID = "peeves_ow02";
		break;
	case 2: 
		sSoundID = "peeves_ow03";
		break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	peevesVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( peevesVoice, SLOT_Talk); 
}


function PeevesYell()
{

	sSoundID = "peeves_ow_long";

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	peevesVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( peevesVoice, SLOT_Talk); 
}


function PeevesLaughing()
{
	switch( Rand(3) )
	{
	case 0: 
		sSoundID = "PC_PVS_happy01fx";
		break;
	case 1: 
		sSoundID = "PC_PVS_happy02fx";
		break;
	case 2: 
		sSoundID = "PC_PVS_happy03fx";
		break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	peevesVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( peevesVoice, SLOT_Talk); 
}


function PeevesTaunting()
{
	switch( Rand(2) )
	{
	case 0: 
		sSoundID = "PC_PVS_Chal2Skurge_23";
		break;
	case 1: 
		sSoundID = "PC_PVS_Chal2Skurge_24";
		break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	peevesVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( peevesVoice, SLOT_Talk); 
}


function StopPeevesDialog()
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("Stopping Dialog");

	StopSound(peevesVoice, SLOT_Talk);
}


function Trigger( actor Other, pawn EventInstigator )
{
	if ( type == Annoyance )
	{

	}
	else
	{
		TriggerEvent( 'BossPeevesTrigger', self, None );
		gotostate('MoveToPoint');
	}
}


function GotoFirstPoint()
{
	local PatrolPoint	tempPatrolPoint;
	local float			fDist, fClosestDist;
	local PatrolPoint	ClosestPoint;
	local int			count;

	fClosestDist = 1000000;

	// Check each point and find the one closest to Peeves
	foreach AllActors(class'PatrolPoint', tempPatrolPoint)
	{
		fDist = VSize( Location - tempPatrolPoint.Location );
	
		if( fDist < fClosestDist )
		{
			fClosestDist = fDist;
			ClosestPoint = tempPatrolPoint;
		}
	}

	currentPoint = ClosestPoint;

}

function GotoNextPoint()
{
	currentPoint = currentPoint.NextPatrolPoint;
}

function FindClosestDeathPoint()
{
	local PatrolPoint	tempPatrolPoint;
	local float			fDist, fClosestDist;
	local PatrolPoint	ClosestPoint;
	local int			count;

	fClosestDist = 1000000;

	// Check each point and find the one closest to Peeves
	foreach AllActors(class'PatrolPoint', tempPatrolPoint, DeathPatrolPoint)
	{
		fDist = VSize( Location - tempPatrolPoint.Location );
	
		if( fDist < fClosestDist )
		{
			fClosestDist = fDist;
			ClosestPoint = tempPatrolPoint;
		}
	}

	currentPoint = ClosestPoint;

	if ( currentPoint == None )
			log(self.name$" has no patrol point to go to. Please see properties!!");

}


function PlayerCutCapture()
{
	gotoState('CutIdle');
}

function PlayerCutRelease()
{
	if ( bGameOver == false )
	{
		gotoState('stateWaitForTrigger');
	}
	else
	{
		gotoState('stateDestroyPeeves');
	}

}

//*************************************************************************************************************************
//This is for where the camera should be looking.
function vector GetCamTargetLoc()
{
	local vector v;
	local vector v1, v2, vH;
	local vector vLoc;

	vLoc = Location;

	return vLoc;
}

//*************************************************************************************************************************
//This one is for where harry should be shooting his spells
function vector GetTargetLocation()
{
	return location;
}


//******************************************************************************************
//This is where harry will face.
function vector GetHarryFaceLocation()
{
	return peevesTrigger.Location;
}


//******************************************************************************************
//This one is for the loc that harry should be moving around
function vector GetHarryMovementCenter()
{
	return peevesTrigger.Location;
}


// --------------------------------------------------------------------------------------------
// *** States
auto state stateIdle
{
	begin:

	if ( type == Annoyance )
	{
		// set bThrowItem to true so he will have an initial velocity
		gotoState('stateAnnoyHarry');
	}
	else
	{
		LoopAnim('idle');
		gotoState('stateWaitForTrigger');
	}
}

state CutIdle
{
	begin:

	loopAnim('idle');

	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);

}

state MoveToPoint
{
	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		MoveSmooth( vDir * airSpeed * DeltaTime);

		if ( vSize2D(location - currentPoint.location) < 10 )
		{
			gotoState('stateTormentHarry');
		}

		//Turn toward Harry
		vTargetDir = normal( playerHarry.location - location );
		desiredRotation = rotator(vTargetDir);
	}

	begin:

	vDir = normal(currentPoint.location - location);

	loopAnim('flying');

}

state stateTormentHarry
{
	begin:

	// Wait for a bit to give user a chance to hit you
	sleep(0.7);

	loopAnim('idle');

//	MoveToward(CurrentPoint);

	//Turn toward Harry
	vTargetDir = normal( playerHarry.location - location );
	desiredRotation = rotator(vTargetDir);

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	switch( Rand(3) )
	{
		case 0:
			PeevesTaunting();
			playAnim('Taunt');
			break;
		default: 
			PeevesLaughing();
			if ( rand(2) == 0 )
			{
				playAnim('taunt_2');
			}
			else
			{
				playAnim('taunt_3');
			}
			sleep(1.0);
			break;
	}

	sleepTime = GetSoundDuration(peevesVoice);

	sleep(sleepTime-2.0f);

	loopAnim('flying');

	gotoState('stateHitHarryLots');
	
}

state stateHitHarryLots
{

	begin:

	randSpells = rand(2) + 1;

	for (spellCount=0; spellCount<=randSpells; spellCount++ )
	{
		//Turn toward Harry
		vTargetDir = normal( playerHarry.location - location );
		desiredRotation = rotator(vTargetDir);

		PlayAnim('Throw',1.2);
		Sleep(0.68);

		if ( rand(2) == 0 )
		{
			throwSound = sound'HPSounds.Critters_sfx.peeves_throw';
		}
		else
		{
			throwSound = sound'HPSounds.Critters_sfx.peeves_throw2';
		}
				
		PlaySound( throwSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );


		SpawnSpell(class'spellEcto',playerHarry);

		//Sleep(1.0);
		Sleep(0.3);

	}

	loopAnim('flying');

	GotoNextPoint();

	GotoState('MoveToPoint');
}


state stateWaitForTrigger
{

	begin:

	loopAnim('idle');

	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);	
}

state stateRunAway
{
begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

//	MoveTo(vHome);
}

// The only difference between stateGoHome and stateRunAway is that bPouting is not set to true. 
// Peeves will stay 'home' until the trigger fires again. 
state stateGoHome
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

//	MoveTo(vHome);
}


// Peeves has been hit by the Skurge spell
state stateHitBySpell
{
	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("In state HIT BY SPELL");
	
	Health -= HitDamage;

	// Moved this up here. Apparently if you work to get another hit on Peeves the game could freeze
	if ( Health <= 0 )
	{
		eVulnerableToSpell=SPELL_None;
	}

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// stop all dialog
	StopPeevesDialog();

	// play his ouch sfx
	PeevesOuch();

	playAnim('preFly');
	FinishAnim();

	if ( Health <= 0 )
	{
		bGameOver=true;
		PeevesYell();
		SendDefeatedTrigger();
		playerHarry.StopBossEncounter();
	}
	else
	{
		//Exit Trigger 
		GotoNextPoint();
		GotoState('MoveToPoint');
	}
}

state stateAnnoyHarry
{
	function BeginState()
	{
//		SetPhysics(PHYS_Flying);
		loopAnim('flying');
	}

	begin:

	playAnim('taunt_2');

	PeevesLaughing();

	sleep(1.0);

	SetPhysics(PHYS_Flying);

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	sleep(0.5);

	//Turn toward Harry
	vTargetDir = normal( playerHarry.location - location );
	desiredRotation = rotator(vTargetDir);

	// Do an annoying animation here


	randSpells = rand(1) + 1;

	for (spellCount=0; spellCount<=randSpells; spellCount++ )
	{
		//Turn toward Harry
		vTargetDir = normal( playerHarry.location - location );
		desiredRotation = rotator(vTargetDir);

		PlayAnim('Throw',1.2);
		Sleep(0.68);

		if ( rand(2) == 0 )
		{
			throwSound = sound'HPSounds.Critters_sfx.peeves_throw';
		}
		else
		{
			throwSound = sound'HPSounds.Critters_sfx.peeves_throw2';
		}
				
		PlaySound( throwSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );


		SpawnSpell(class'spellEcto',playerHarry);

		//Sleep(1.0);
		Sleep(0.3);

	}

	gotoState('GoAway');


}

state DontAnnoyHarry
{

	begin:

	playAnim('preFly');
	FinishAnim();

	PeevesOuch();

	gotoState('GoAway');

}

state GoAway
{

	begin:

	loopAnim('flying');

	MoveTo(CurrentPoint.location);

	if ( currentPoint.NextPatrolPoint == None )
	{
		gotoState('stateDestroyPeeves');
	}
	else
	{
		currentPoint = currentPoint.NextPatrolPoint;

		// Stop moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);

		//Turn toward Harry
		vTargetDir = normal( playerHarry.location - location );
		desiredRotation = rotator(vTargetDir);

		PeevesLaughing();

		// Play funny anim here
		playAnim('taunt_3');
		finishAnim();

		gotoState('GoAway');

	}
}


// MINIBOSS
// bGameOver is set to true when health is <= 0. The next time a cutscene releases (in PlayerCutRelease)
// Peeves will be sent to this state and destroyed. 
// ANNOYANCE
// Peeves will run to his DeathPatrolPoint and if there is no next point he will be destroyed. If there
// is a next point he will face Harry, play an animation, yell something and run to the next point
state stateDestroyPeeves
{
	begin:

	ParticleFXActor.ShutDown();
	sleep(0.2);
	Destroy();
}




defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skpeevesMesh'
	DrawType=DT_Mesh
	Menuname="Peeves"
	eVulnerableToSpell=SPELL_Skurge
	Physics=PHYS_FLYING
	AmbientGlow=65
	CollisionRadius=40
    CollisionHeight=40

	SightRadius=4000
	PeripheralVision=0

	bCollideWorld=False
	bCollideActors=True
	bBlockActors=False
    bProjTarget=True
    RotationRate=(Pitch=80000,Yaw=80000,Roll=80000)
	bGameOver=False

	airSpeed=150

	fParticleTrailLife=1.0

	HitDamage=20

    EnemyHealthBar=EnemyBar_Peeves
}


// --------------------------------------------------------------------------------------------
// Peeves.uc - End of file   
// --------------------------------------------------------------------------------------------

