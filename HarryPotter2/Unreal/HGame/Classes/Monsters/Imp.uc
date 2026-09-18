class Imp extends HChar;

/////////////////////////////////////////////////////
//
//	The behavior of an Imp
//
// Run around on a patrol path  (using patrol points) until Harry
// gets within sightRadius and then will run out and bite. Can be 
// picked up and thrown by Harry. 
//
/////////////////////////////////////////////////////


// *** Variables
var vector		
vHome;			// starting position
var name  savedState;

var vector		vTargetDir;		// stores our target direction (normalized)
//var rotator		rot;
var float	    attackDistance;
var bool		bPlayedWarning;
var bool		flag;
var float		randomRunningSpeed;
var float		runningForHarry;

var sound impTalkSound;
var sound impStruggleSound;

var vector  vTemp;

var() int numAttacksDefault;
var   int numAttacks;

var() float fDamageAmount;
var() name  groupName;

var() float	timeStunnedDefault;
var float timeStunned;
var bool bStunned;
var bool bCarried;

var() float timeWarningWhileCarried;

var() bool  bWaitForTrigger;
var float timeIdleFidgit;
var bool  bFidgit;


// *** Constants
const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


// --------------------------------------------------------------------------------------------
// *** Functions

// *** generic

function PreBeginPlay()
{
	Super.PreBeginPlay();
	
	// set our home location to be were we started
	vHome = location;

	timeStunned = timeStunnedDefault;
	randomRunningSpeed = (groundRunSpeed-10) + FRand()*35;
	runningForHarry = 5;
	
	bFlipPushable	= true;
	lockSpell		= true;

	// have idle the default animation
	LoopAnim('idle');
}

function PostBeginPlay()
{
	local Imp tempImp;

	Super.PostBeginPlay();

	numAttacks = numAttacksDefault;
	vHome = location;

	attackDistance = playerHarry.collisionRadius + collisionRadius + 5;

//	AmbientSound = impTalkSound;

}


function PlayerCutCapture()
{
	savedState = GetStateName();

	gotoState('CutIdle');
}


state CutIdle
{
	begin:
	
	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);

	loopAnim('idle');

}

function PlayerCutRelease()
{
	gotoState(savedState);
}

// *** Sound

// Play the sound the imp makes when hit by a spell
function playHitSound()
{
	local sound hitSound;
	local int randNum;

	randNum = rand(5);

	switch (randNum)
	{
	case 0:
		hitSound = sound'HPSounds.Critters_sfx.imp_Ouch_01';
		break;
	case 1:
		hitSound = sound'HPSounds.Critters_sfx.imp_Ouch_02';
		break;
	case 2:
		hitSound = sound'HPSounds.Critters_sfx.imp_Ouch_03';
		break;
	case 3:
		hitSound = sound'HPSounds.Critters_sfx.imp_Ouch_04';
		break;
	case 4:
		hitSound = sound'HPSounds.Critters_sfx.imp_Ouch_05';
		break;
	default:
		// just in case the skies fall
		hitSound = sound'HPSounds.Critters_sfx.imp_Ouch_01';
		break;
	}

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );


}

// Play the sound the imp makes when they fall to the ground
function playDieSound()
{
	local sound hitSound;
	local int randNum;

	randNum = rand(3);

	switch (randNum)
	{
	case 0:
		hitSound = sound'HPSounds.Critters_sfx.imp_Die_01';
		break;
	case 1:
		hitSound = sound'HPSounds.Critters_sfx.imp_Die_04';
		break;
	case 2:
		hitSound = sound'HPSounds.Critters_sfx.imp_Die_05';
		break;
	default:
		// just in case the skies fall
		hitSound = sound'HPSounds.Critters_sfx.imp_Die_01';
		break;
	}

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );

}

// Play the sound the imp makes when they start to attack
function playAttackSound()
{
	local sound hitSound;
	local int randNum;

	randNum = rand(5);

	switch (randNum)
	{
	case 0:
		hitSound = sound'HPSounds.Critters_sfx.imp_Attack_01';
		break;
	case 1:
		hitSound = sound'HPSounds.Critters_sfx.imp_Attack_02';
		break;
	case 2:
		hitSound = sound'HPSounds.Critters_sfx.imp_Attack_03';
		break;
	case 3:
		hitSound = sound'HPSounds.Critters_sfx.imp_Attack_04';
		break;
	case 4:
		hitSound = sound'HPSounds.Critters_sfx.imp_Attack_05';
		break;
	default:
		// just in case the skies fall
		hitSound = sound'HPSounds.Critters_sfx.imp_Attack_01';
		break;
	}

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );

}

// *** other

function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellFlipendo( spell, vHitLocation );
	
	gotostate('stateHitByFlipendo');
	
	return true; // true == create spell effects
}


function Landed( vector HitNormal )
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": In function Landed" );

	Super.Landed(HitNormal);

	// if the imp is screaming to get away stop the sound
	StopSound( impStruggleSound, SLOT_Misc );

	if ( IsInState('stateGetAwayFromHarry') || IsInState('stateBeingCarried') )
	{
		gotostate('stateGotAwayFromHarry');
	}


	if ( !IsInState('stateIdle') )
	{
//		SetPhysics(PHYS_Walking);
//		gotoState('HitGround');
	}
}



function ThrownLanded(vector HitNormal)
{
	local float newR, newH;

	newR = Default.CollisionRadius * DrawScale / Default.DrawScale;
	newH = Default.CollisionHeight * DrawScale / Default.DrawScale;

	// do not allow collisionHeight be less then 13,
	// otherwise Harry can step on Imp
	if(newH < 13)
		newH = 13;

	// Reset the collision size
	SetCollisionSize(newR, newH);

	// if the imp is screaming to get away stop the sound
	StopSound( impStruggleSound, SLOT_Misc );

	if ( !bDespawned )
	{
		gotoState('HitGroundWhenThrown');
	}
	else
	{
		if( GetStateName() != 'stateDied' )
			gotostate('stateDied');
	}
}


function bool GoAfterHarry()
{
	local bool bRet;
	local vector vVectorToHarry;

	bRet = false;

	vVectorToHarry = playerHarry.location - location;

	if ( vsize2D(vVectorToHarry) < SightRadius )
	{
		bRet = true;
	}

	return bRet;

}

// Returns true if the imp is in a position to attack harry.
function bool ReadyPosition()
{
	if ( vSize2D(playerHarry.location - location) < attackDistance ) 
		return true;

	return false;
}

function Trigger( actor Other, pawn EventInstigator )
{
	gotoState('RandomWait');
}


function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	// Check if the imp is stunned
	if ( bStunned == true )
	{
		timeStunned -= DeltaTime;

		// Check if the imp is being carried
		if ( bCarried == true )
		{
			// Start playing the struggle animation to warn the user before time runs out
			if ( timeStunned <= timeWarningWhileCarried && bPlayedWarning == false )
			{
				bPlayedWarning = true;
				impStruggleSound = sound'HPSounds.Critters_sfx.imp_Attack_01';
				PlaySound( impStruggleSound, SLOT_Misc, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, true );
			//	playAttackSound();
				loopAnim('Struggle');
			}

			// If no longer stunned bite Harry and run away
			if ( timeStunned <= 0 )
			{
				gotoState('stateGetAwayFromHarry');
			}
		
		}
		else
		{
			// If no longer stunned run back home and look for harry again
			if ( timeStunned <= 0 )
			{
				gotostate('stateUpFromStunned');
			}
		}

	}

	// Check if you should go after harry
	if ( IsInState('Patrol') )
	{
		if ( GoAfterHarry() )
		{
			vHome = location;
			gotostate('stateMoveTowardHarry');
		}
	}
	
	// Check if the imp has been picked up
	if ( bObjectCanBePickedUp == true )
	{
		// the only way to support Harry holding the imp
		if((owner == playerHarry) && (GetStateName() != 'stateBeingCarried') && !IsInState('stateGetAwayFromHarry'))
		{
			gotostate('stateBeingCarried');
		}
	}

	if ( IsInState('stateMoveTowardHarry') )
	{
		runningForHarry -= DeltaTime;

		if ( runningForHarry <= 0 )
		{
			gotoState('takeABreather');
		}
	}

	// Check if the imp has run outside of it's sightRadius from vHome
	if ( (IsInState('stateMoveTowardHarry') || 
		 IsInState('stateBiteHarry'))
		 && vSize(location - vHome) > sightRadius )
	{
		gotoState('HarryGotAway');
	}

	// Check if the imp is in a position to bite Harry
	if ( IsInState('stateMoveTowardHarry') && ReadyPosition() ==  true )
	{
		gotoState('stateBiteHarry');
	}	
}



// --------------------------------------------------------------------------------------------
// *** States
auto state stateIdle
{

	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": auto stateIdle" );

	sleep(0.2);
	
	// set anim to walk
	groundSpeed = groundWalkSpeed;
	LoopAnim('walk');

	if ( bWaitForTrigger == true )
	{
		// Wait for a sign
		gotoState('stateWaitForTrigger');
	}
	else
	{
		// set anim to walk
		groundSpeed = groundWalkSpeed;
		LoopAnim('walk');

		// Start moving on the patrol path
		gotostate('RandomWait');
	}

}

state RandomWait
{
	begin:

	sleep(Frand()+0.5);

	// Start moving on the patrol path
	gotostate('Patrol');
}


state stateMoveTowardHarry
{
	function BeginState()
	{
		runningForHarry = 5;
	}

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": state MoveTowardHarry" );

	// Stop moving
	velocity = vec(0,0,0);
	acceleration = vec(0,0,0);

	// while rotating use this animation
	LoopAnim('fidget_1');
	sleep(1.0);

	// Play the sound they make before they attack
	playAttackSound();

	groundSpeed = randomRunningSpeed;
	LoopAnim('run',1.5);

loop:

	MoveToward(playerHarry);
	sleep(0.2);

goto 'loop';

}


state takeABreather
{
	function BeginState()
	{
		runningForHarry = 5;
	}
	begin:

	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);

	loopAnim('fidget_1');

	sleep(frand()+2.5);

	gotoState('stateMoveTowardHarry');

}


state stateBiteHarry
{

	begin:

	// Stop moving
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	//Turn toward Harry (since the imp is so small don't turn him directly toward Harry or he will look up
	vTargetDir = playerHarry.location;
	vTargetDir.z = location.z;
	TurnTo(vTargetDir);

	if ( vSize(playerHarry.location - location) < attackDistance+12 )
	{	
		PlayAnim( 'attack',2.0 );
		sleep(0.4);

		if ( vSize(playerHarry.location - location) < attackDistance+12 )
		{
			playerHarry.TakeDamage (fDamageAmount, Pawn(Owner), location, velocity*1, 'Imp');
		}
	}
	else
	{
		playAnim('fidget_1');
		FinishAnim();
	}
	
	sleep(0.2);

	gotostate('stateRunAway');

}


state stateRunAway
{
	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateRunAway" );

	// run scared!!!
	LoopAnim('run',1.5);

	flag = false;

	while ( vSize2D(location - vHome) > 18 && flag == false )
	{
		MoveTo(vHome);
		sleep(0.1);

		if ( vSize(location - vHome) < sightRadius && ReadyPosition() ==  true )
			flag = true;
	}
	
	// set anim to walk
	groundSpeed = groundWalkSpeed;
	LoopAnim('walk');

	// Go back to patroling after waiting a short random period
	gotostate('RandomWait');
}


state HarryGotAway
{

	begin:

	// Stop moving
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	SetLocation(OldLocation);

	// face Harry
	vTemp = vec(playerHarry.location.x, playerHarry.location.y, location.z);
	vTargetDir		= normal(vTemp - location);
	desiredRotation = rotator(vTargetDir);

	// play Taunt Anim
	loopAnim('fidget_1');

	// yell like you've been hit (may change to angry if I get it)
	playHitSound();

	// hang out for a while
	sleep(1.5);

	// Run back home and look for harry again
	gotostate('stateRunAway');

}


state stateBeingThrown
{
	function BeginState()
	{
		SetCollisionSize(5,13);
	}

	begin:

	// if the imp is screaming to get away stop the sound
	StopSound( impStruggleSound, SLOT_Misc );

	PlayAnim('thrown' );
	FinishAnim();

	loopAnim('thrownLoop');

	sleep(3);
}


state stateHitByFlipendo
{

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateHitByFlipendo" );

	// Play the sound of getting hit
	playHitSound();

	if ( --numAttacks <= 0 )
	{

		// reset the number of attacks needed to the default
		numAttacks = numAttacksDefault;

		// The imp is stunned
		bStunned = true;

		// The imp can be picked up by Harry
		bObjectCanBePickedUp=true;

		// Reset the amount of time stunned
		timeStunned = timeStunnedDefault;

		// Play the sound when the fall to the ground
		playDieSound();

		// Play the knockback animation
		PlayAnim('knockback');
		FinishAnim();

	}
	else
	{

		// Play the knockback animation
		PlayAnim('knockback');
		FinishAnim();

		// Sit there a second
		LoopAnim('stunned');
		sleep(0.1);

		// Jump up to run away
		PlayAnim('wakingup2run');
		FinishAnim();

		// Run back home and look for harry again
		gotostate('stateRunAway');

	}

}


state stateUpFromStunned
{

	begin:

	bObjectCanBePickedUp=false;
	bStunned = false;
	numAttacks = numAttacksDefault;

	PlayAnim('wakingup2run');
	FinishAnim();

	// Run back home and look for harry again
	gotostate('stateRunAway');

}


state HitGround
{

	begin:

	loopAnim('Stunned');

}


state HitGroundWhenThrown
{
	function BeginState()
	{
		// Reset the amount of time stunned
		timeStunned = timeStunnedDefault;

		bStunned = true;

		// You can pick up the imp 
		bObjectCanBePickedUp=true;
	}

	begin:

	// Play the ouch sound (since it would hurt to be thrown)
	playHitSound();

	loopAnim('Stunned');

}


state stateBeingCarried
{
	function BeginState()
	{
		bCarried = true;
	}

	function EndState()
	{
		bCarried = false;
		bStunned = false;
		bPlayedWarning = false;
		bObjectCanBePickedUp=false;

	}


	begin:

	loopAnim('Carried');

}


state stateGetAwayFromHarry
{

	begin:

	PlayAnim( 'attack',2.0 );
	sleep(0.4);

	playerHarry.TakeDamage (fDamageAmount, Pawn(Owner), location, velocity*1, 'Imp');

}


state stateGotAwayFromHarry
{


	begin:

	loopAnim('fidget_1');

	// face Harry
	vTemp = vec(playerHarry.location.x, playerHarry.location.y, location.z);
	vTargetDir		= normal(vTemp - location);
	desiredRotation = rotator(vTargetDir);

	sleep(1.5);

	// Run back home and look for harry again
	gotostate('stateRunAway');

}


state stateWaitForTrigger
{
	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		timeIdleFidgit -= DeltaTime;

		if ( timeIdleFidgit <= 0 )
		{
			bFidgit = true;
		}
	}

	begin:

	loopAnim('idle');

loop:

	if ( bFidgit == true )
	{
		bFidgit = false;

		playAnim('fidget_1');
		FinishAnim();

		timeIdleFidgit = 10;

		loopAnim('idle');
	}

	sleep(0.5);

goto 'loop';

}

state stateDied
{
	begin:

	PlaySound( sound'HPSounds.Critters_sfx.horklump_mushroom_head_explode', SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]70000, [Pitch]RandRange(0.8, 1.2),, false );

	if( Rand(2) == 0)
		spawn(class'dustcloud01_tiny', self, ,location, rotation);
	else
		spawn(class'dustcloud02_small',self, ,location, rotation);

	Destroy();

}

defaultproperties
{
	SightRadius=600

    bStatic=false

	GroundSpeed=150
	groundRunSpeed=150
	airSpeed=150
	PeripheralVision=1
	Physics=PHYS_Walking
	eVulnerableToSpell=SPELL_Flipendo
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skImpMesh'
	DrawScale=1.30
	Fatness=140
	AmbientGlow=75
	CollisionRadius=10
	CollisionHeight=15
	RotationRate=(Pitch=200000,Yaw=200000,Roll=200000)
	
	bCollideActors=true
	bCollideWorld=true
	bBlockActors=true
	bBlockPlayers=true
	
	bProjTarget=true

	numAttacksDefault=1
	timeStunnedDefault=20
	fDamageAmount=2
//	distanceStunnedWhileCarried=250
//	timeStunnedWhileCarriedDefault=10
	timeWarningWhileCarried=5
	timeIdleFidgit=10
	bAccurateThrowing=True

	bObjectCanBePickedUp=false
	bAccurateThrowing=true
	bDespawnable=true
    bThrownObjectDamage=True


	impTalkSound=Sound'HPSounds.Critters_sfx.pixie_dust_loop'

}

// --------------------------------------------------------------------------------------------
// Imp.uc - End of file   
// --------------------------------------------------------------------------------------------

