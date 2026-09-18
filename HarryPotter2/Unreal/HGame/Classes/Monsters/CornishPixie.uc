class CornishPixie extends HChar;

/////////////////////////////////////////////////////
//
//		The behavior of a cornish pixie
//
//	COMPLETELY NEW BEHAVIOR! IF YOU WANT THE OLD BEHAVIOR GET IT OUT OF SOURCESAFE
//
// Flies around on a spline trailing pixie dust until:
//  1. Harry gets within sight radius (can be changed by Level designers)
//  2. Harry hits a pixie (while running or jumping)
//
//	Attack:
//	Will come toward Harry from the front and when within a certain distance will 
//	cause damage. They will not attach to Harry or hit him from behind. After 
//	attacking once they will return to their spline. Only after returning to 
//	their spline can they look to attack again.	The pixies will only follow 
//	within their sight distance from the point they began their attack. Also,
//	if Harry turns to run away they will abort their attack. If there are two or 
//	more pixies in an area the level designers can put them in groups and only 
//	one pixie per group will attack at a time. The exception to this is if 
//	Harry encroaches on the pixie. If Harry is closer than 'encroachRadius' the pixie 
//	will attack no matter what (if you run/jump into them). 
//
//	Trigger:
//	Level Designers can trigger the pixie using a standard trigger. Until it's triggered
//	the pixie will loop its' idle animation. Once triggered it will either go directly
//	to it's spline or it will go to a patrol point place by the level designer. When the 
//	pixie get to the patrol point it will look for Harry. If Harry is close the pixie will 
//	attack him. After the attack it will return to the patrol point and look for Harry again.
//	If Harry is not close it will go to its spline. 
//
// Can be hit with the Rictusempra spell: (Number of hits can be changed by level designers)
//  1 hit: Leave Harry alone and go back to a spline. Once
//		   it reaches the spline it will look for Harry again
//  2 hit: Fall to the ground stunned. Will reanimate after t seconds
//
/////////////////////////////////////////////////////


// *** Variables
var vector		vHome;			// starting position
var vector		vOriginalHome;// original starting position

var vector		vTargetDir;		// stores our target direction (normalized)
var rotator		rHitRotation;	// Need to reset the pitch to 0 when spelled
var float		DistanceHome;

var (VisualFX)ParticleFX		fxFlyParticleEffect;			
var (VisualFX)class<ParticleFX>	fxFlyParticleEffectClass;

var ParticleFX fxBlowUp;
var ParticleFX fxHit;

var baseWand wand;

var sound pixieLoopSound;

var vector  vTemp;
var CornishPixie myFriends[3];
var int			 numFriends;
var int			 counter;
var bool		 bAttacking;
var vector		 vHarryAttackPosition;
var float		 randomTalk;

var() int numAttacksDefault;	// The number of attacks before getting stunned to the ground
var   int numAttacks;

var() float fDamageAmount;		// The amount of damage per hit
var() name  groupName;			// The group name of the pixie (pixie in the same area have the same group name)

var() float	timeStunned;		// The amount of time stunned before getting up again
var bool bStunned;

var() float encroachRadius;		// Attack Harry if he comes within this distance (different than sightRadius)
var() name	patrolPointTag;		// Name of the patrol point the pixie should go to (only if waitForTrigger is true)
var() bool  waitForTrigger;		// Wait for a trigger before moving
var() bool  goToPatrolPoint;	// Go to this patrol point and look for Harry
var	  PatrolPoint  pp;			// The patrol point

var() float StayOnSplineDefault;// The amount of time the pixie will hang on the spline before attacking Harry again. 
var float StayOnSpline; 

var() float StopAttackDistance;//  The distance where the pixie should stop following. 

// *** Constants
const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


// --------------------------------------------------------------------------------------------
// *** Functions

function PreBeginPlay()
{
	Super.PreBeginPlay();
	
	// set our home location to be were we started
	vHome = location;
	vOriginalHome = location;
	
//	bFlipPushable	= true;
	lockSpell		= true;

	// have idle the default animation
	LoopAnim('idle');
}

function PostBeginPlay()
{
	local CornishPixie tempPixie;
	local PixieMarker marker;

	Super.PostBeginPlay();

	if ( DrawScale != Default.Drawscale )
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);
	}

	numAttacks = numAttacksDefault;
	SetCollision(true,false,true);

	// Look for the Pixies with the same group
	foreach AllActors(class'CornishPixie', tempPixie)
	{
		if ( tempPixie != self )
		{
			if ( tempPixie.groupName == groupName )
			{
				myFriends[numFriends] = tempPixie;
				numFriends++;
			}
		}
	}
}


function PlayerCutCapture()
{
	gotoState('CutIdle');
}

state CutIdle
{
	begin:

	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);

	// just play the idle until the cut scene is done
	GotoState('waitingForTrigger');
}

function PlayerCutRelease()
{
	loopAnim('fly');

	// Return to the spline
	GotoState('stateLoopSplinePath');
}


function timer()
{
	// CG wants the pixies to explode into pixie dust when they are hit so 
	gotoState('BlowUpAndDie');

}


function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellRictusempra( spell, vHitLocation );
	
	gotostate('stateHitByRictusempra');
	
	return true; // true == create spell effects
}


function Landed( vector HitNormal )
{

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": In function Landed" );

	SetTimer(0,false);

	Super.Landed(HitNormal);

	gotoState('HitGround');

}

function bool GoAfterHarry()
{
	local bool bRet;
	local vector vVectorToHarry;

	bRet = false;

	vVectorToHarry = playerHarry.location - location;

	// Check if Harry is close enough to attack
	if ( vsize(vVectorToHarry) < SightRadius && !IsInState('CutIdle') )
	{
		bRet = true;
	}

	return bRet;

}

// Play the sound the pixie makes when just talking
function playTalkSound()
{
	local sound talkSound;
	local int randNum;

	randNum = rand(6);

	switch (randNum)
	{
	case 0:
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_01';
		break;
	case 1:
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_02';
		break;
	case 2:
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_03';
		break;
	case 3:
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_04';
		break;
	case 4:
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_05';
		break;
	case 5:
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_06';
		break;
	default:
		// just in case the skies fall
		talkSound = sound'HPSounds.Critters_sfx.PIX_talk_06';
		break;
	}

	PlaySound( talkSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]3000, [Pitch]RandRange(0.8, 1.2),, false );

}

// Play the sound the pixie makes when attacking or about to attack (also played for hitting the ground)
function playAttackSound()
{
	local sound attackSound;
	local int randNum;

	randNum = rand(5);

	switch (randNum)
	{
	case 0:
		attackSound = sound'HPSounds.Critters_sfx.PIX_attack_01';
		break;
	case 1:
		attackSound = sound'HPSounds.Critters_sfx.PIX_attack_02';
		break;
	case 2:
		attackSound = sound'HPSounds.Critters_sfx.PIX_attack_03';
		break;
	case 3:
		attackSound = sound'HPSounds.Critters_sfx.PIX_attack_04';
		break;
	case 4:
		attackSound = sound'HPSounds.Critters_sfx.PIX_attack_05';
		break;
	default:
		// just in case the skies fall
		attackSound = sound'HPSounds.Critters_sfx.PIX_attack_05';
		break;
	}

	PlaySound( attackSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );

}


// Play the sound the pixie makes when hit by a spell
function playHitSound()
{
	local sound hitSound;
	local int randNum;

	randNum = rand(6);

	switch (randNum)
	{
	case 0:
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch1';
		break;
	case 1:
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch2';
		break;
	case 2:
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch3';
		break;
	case 3:
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch4';
		break;
	case 4:
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch5';
		break;
	case 5:
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch6';
		break;
	default:
		// just in case the skies fall
		hitSound = sound'HPSounds.Critters_sfx.pixie_ouch1';
		break;
	}

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );

}

// Play the sound the pixie makes when hit by a spell
function playBiteSound()
{
	local sound hitSound;
	local int randNum;

	randNum = rand(5);

	switch (randNum)
	{
	case 0:
		hitSound = sound'HPSounds.Critters_sfx.PIX_Bite1';
		break;
	case 1:
		hitSound = sound'HPSounds.Critters_sfx.PIX_Bite2';
		break;
	case 2:
		hitSound = sound'HPSounds.Critters_sfx.PIX_Bite3';
		break;
	case 3:
		hitSound = sound'HPSounds.Critters_sfx.PIX_Bite4';
		break;
	case 4:
		hitSound = sound'HPSounds.Critters_sfx.PIX_Bite5';
		break;
	default:
		// just in case the skies fall (again)
		hitSound = sound'HPSounds.Critters_sfx.PIX_Bite1';
		break;
	}
		

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );

}

// --------------------------------------------------------------------------------------------
// *** States
auto state stateIdle
{

	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": auto stateIdle" );
	
	// set anim to fly
	LoopAnim('fly');

	if ( waitForTrigger == true )
	{
		gotoState('waitingForTrigger');
	}
	else
	{
		// Start moving on the spline
		gotostate('stateLoopSplinePath');
	}

}

state stateLoopSplinePath
{

	function BeginState()
	{
		loopAnim('fly');
		AmbientSound = pixieLoopSound;
		StayOnSpline = StayOnSplineDefault;
		eVulnerableToSpell = SPELL_None;
//		SetCollision([NewBlockActors]false);

		// start the pixie dust
		fxFlyParticleEffect = spawn( fxFlyParticleEffectClass,,,Location );
	}

	function EndState()
	{
		if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": EndState : stateLoopSplinePath" );
		// Get rid of the spline so the pixie can 'fly free'
		DestroyControllers();
		AmbientSound = None;
		SetPhysics(PHYS_Flying);
//		SetCollision(true,true,true);
		bCollideWorld = true;
		bAlignBottom=false;

		// shutdown the flying pixie dust
		fxFlyParticleEffect.ShutDown();
	}

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		StayOnSpline -= DeltaTime;

		// When the pixie first gets on the spline wait a bit. Creating and destroying the IP manager
		// causes a crash if it's done too fast...I think
		if ( StayOnSpline < 0 )
		{

			eVulnerableToSpell=SPELL_Rictusempra;

			if ( GoAfterHarry() )
			{
				gotostate('stateMoveTowardHarry');
			}
			else
			{
				randomTalk -= DeltaTime;

				if ( randomTalk < 0 )
				{
					randomTalk = fRand() * 5 + 1;
					playTalkSound();
				}
			}

		}

		// If you run into Harry just attack him
		if ( Vsize(playerHarry.location-location) < encroachRadius )
		{
			gotoState('stateAttackHarry');
		}


		// Update particles
		fxFlyParticleEffect.SetLocation( Location );
	}

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateLoopSplinePath" );

	FollowSplinePath();
}


state stateMoveTowardHarry
{
	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": state MoveTowardHarry" );

	SetPhysics(PHYS_Flying);

	vHome = location;

	// while rotating use this animation
//	LoopAnim('taunt');
	playAttackSound();
//	sleep(1.5);

	gotoState('stateAttackHarry');

}


state stateAttackHarry
{
	function Tick(float DeltaTime)
	{

		Super.Tick(DeltaTime);

		// Check if Harry is running away or if you're somehow behind him
//		if ( normal(playerHarry.location - location) dot vector(playerHarry.rotation) > 0)
//		{
//			gotoState('HarryGotAway');
//		}

		
		// Check if you're close enough to Harry to damage him
		// For some reason I'm not getting a touch
//		if ( vsize(location - playerHarry.location) <= (playerHarry.collisionRadius+collisionRadius+15) )
		if ( vsize(location - playerHarry.location) <= (playerHarry.collisionRadius+collisionRadius+5) )
		{
			// You're close. Damage Harry
			if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
			{
				gotoState('DamageHarry');
			}
		}

		// Check if Harry is too far away to attack. The test is done from where he started
	//	if ( vsize(vHome - playerHarry.location) > sightRadius )
		if ( vsize(vHome - playerHarry.location) > StopAttackDistance )
		{
			// stop
			Velocity = vect(0,0,0);
			Acceleration = vect(0,0,0);

			gotoState('HarryGotAway');
		}

	}

	begin:

	LoopAnim('fly');

	loop:

		vHarryAttackPosition = playerHarry.location + (vector(playerHarry.rotation) * (playerHarry.collisionRadius+collisionRadius+5)) + vec(0,0,-25);
		MoveTo(vHarryAttackPosition);
		Sleep(0.1);

	goto 'loop';

}

state DamageHarry
{

	begin:

	// stop
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	playAnim('attack',3.0);
	sleep(0.3);

	playBiteSound();
	sleep(0.1);

	playerHarry.TakeDamage (fDamageAmount, Pawn(Owner), location, velocity*1, 'Pixie');

	gotoState('stateRunAway');

}


state stateRunAway
{
	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateRunAway" );

	// run scared!!!
	LoopAnim('fly');

	while ( vSize(location - vHome) > 35 )
	{
		DistanceHome = vSize(location - vHome);
		MoveTo(vHome);
		sleep(0.2);
		if ( DistanceHome-vSize(location - vHome) < 5 )
		{
			if ( vHome != vOriginalHome )
			{
				// not moving and must be stuck. Make home the original placement of the pixie
				vHome = vOriginalHome;
			}
			else
			{
				MoveTo(playerHarry.location);
				sleep(0.2);
			}

		}
	}
		
	gotostate('stateLoopSplinePath');
}

state stateHitByRictusempra
{

	begin:

	// kill the interpolation manager
	DestroyControllers();

	PlaySound( sound'HPSounds.Critters_sfx.SPI_hit', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]2000, [Pitch]RandRange(1.6, 2.2),, false );

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateHitByRictusempra" );

	if ( --numAttacks <= 0 )
	{

		SetTimer(timeStunned,false);
		bStunned = true;
		fxFlyParticleEffect.ShutDown();

		SetCollision(false,false,false);

		SetCollisionSize(Default.CollisionRadius/5, Default.CollisionHeight-(Default.CollisionHeight-1));
		eVulnerableToSpell=SPELL_None;

	//	bAlignBottom=true;

		// Stop moving when attacked
		velocity = vect(0,0,0);
		acceleration = vect(0,0,0);

		rHitRotation = rotation;
		rHitRotation.pitch = 0;

		DesiredRotation = rHitRotation;
		SetRotation(rHitRotation);

		// Play the ouch sfx
		playHitSound();

		// play the ouch effect
		fxHit = spawn(class'pixiehit', self, ,location, rotation);

		// Play the knockback animation
		loopAnim('stunspin');
		// show the hit effect for a moment
		sleep(0.15);

		// Somehow my bCollideWorld is set to false. Reset here
		bCollideWorld = true;

		// shutdown the hit particle
		fxHit.Shutdown();

		playAttackSound();

		// Fall to the ground. Landed event will send to state HitGround
		// On the weird off chance that there is no ground the timer will bring the pixie back
		SetPhysics(PHYS_Walking);
	//	SetPhysics(PHYS_Falling);

	}
	else
	{

		// Stop moving when attacked
		velocity = vect(0,0,0);
		acceleration = vect(0,0,0);

		// Play the ouch sfx
		playHitSound();

		// Play the knockback animation
		PlayAnim('stun');
		FinishAnim();

		// Sit there a second
		LoopAnim('idle');
		sleep(0.1);

		// Run back home and look for harry again
		gotostate('stateRunAway');

	}

}

state HitGround
{
	begin:

//	PlayAnim('Stunhitground');
//	FinishAnim();

	playHitSound();

	gotoState('BlowUpAndDie');

//	loopAnim('Stunnedground');
}

state HarryGotAway
{

	begin:

	// face Harry
	vTemp = vec(playerHarry.location.x, playerHarry.location.y, location.z);
	vTargetDir		= normal(vTemp - location);
	desiredRotation = rotator(vTargetDir);

	// play Taunt Anim
	loopAnim('taunt');

	// yell like you've been hit (may change to angry if I get it)
	playAttackSound();

	// hang out for a while
	sleep(1.5);

	// Run back home and look for harry again
	gotostate('stateRunAway');

}

state waitingForTrigger
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		gotoState('Triggered');
	}

	begin:

//	loopAnim('idle');
	loopAnim('fly');

}

state Triggered
{

	begin:

	playTalkSound();

	if ( goToPatrolPoint == true )
	{
		// Find the patrol point
		foreach AllActors( class'PatrolPoint', pp, patrolPointTag )
			break;

		loopAnim('fly');

		// Move toward the patrol point
		MoveToward(pp);

		// Get close to it
		while ( vsize(location - pp.location ) > 10 )
		{
			sleep(0.05);
		}

		// Now that you're here look for harry
		if ( GoAfterHarry() )
		{
			gotostate('stateMoveTowardHarry');
		}
		else
		{
			// Can't find Harry so go to the spline
			gotostate('stateLoopSplinePath');
		}

	}
	else
	{
		gotostate('stateLoopSplinePath');
	}
}


state BlowUpAndDie
{

	begin:

	PlaySound( sound'HPSounds.Critters_sfx.horklump_mushroom_head_explode', SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]70000, [Pitch]RandRange(0.8, 1.2),, false );
	
	fxBlowUp = spawn(class'pixieexplode', self, ,location, rotation);

	sleep(0.1);

	if ( fxBlowUp != None )
		fxBlowUp.Shutdown();

	Destroy();

}


defaultproperties
{
	SightRadius=400

    bStatic=false

	GroundSpeed=75
	airSpeed=120
	PeripheralVision=1
	Physics=PHYS_FLYING
	eVulnerableToSpell=SPELL_Rictusempra
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skcornishpixieMesh'
	fxFlyParticleEffectClass=class'PixieFlying'
	DrawScale=2.00
	AmbientGlow=200
	CollisionRadius=30
	CollisionHeight=20
	RotationRate=(Pitch=50000,Yaw=50000,Roll=50000)
	
	bCollideActors=true
	bCollideWorld=true
	bBlockActors=false
	bBlockPlayers=true
	
	bProjTarget=true

	numAttacksDefault=2
	timeStunned=2
	fDamageAmount=2
	encroachRadius=50
	StopAttackDistance=800
	bThrownObjectDamage=True
	randomTalk=frand()*5+1
	StayOnSplineDefault=3

	RunAnimName=fly
	WalkAnimName=fly

	// sounds
	pixieLoopSound=Sound'HPSounds.Critters_sfx.pixie_dust_loop'

}

// --------------------------------------------------------------------------------------------
// CornishPixie.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------

