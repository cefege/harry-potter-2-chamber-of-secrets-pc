//===============================================================================
//  firecrabSmall
//===============================================================================


class firecrabSmall extends firecrab;


// *** Variables

var() bool  bMoveAround;
var spellFireSmall smallSpell;
var vector vTemp, vTemp2;
var rotator rotationChange;
var float normalSpeed;

var() float strafeSpeed;

var() float timeBetweenShots;

var() float fIncreaseHitTimeDistance;
var() float	fHitTimeIncrement;

var() int iAccuracyMin;
var() int iAccuracyMax;

var() bool bPlayRoar;
var() float SpellDamage;

var() int iNumShotsBetweenPreAttack;
var int iNumShots;

var() bool bCanStrafe;

var bool strafeLeft;



// *** Constants

const		BOOL_DEBUG_AI	= true;	// if true this will spit out debug AI info


//** Functions

// When the FireCrab is on his back he is vulnerable to the Flipendo spell
function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellFlipendo( spell, vHitLocation );

// Look for a way to return to the normal force when moving away from a ledge. 
//	fFlipPushForceXY = default.fFlipPushForceXY;
//	fFlipPushForceZ = default.fFlipPushForceZ;

	// Call HandleSpellRictusempra so we only have to update one set of code. 
	if ( !IsInState('stayFlipped') )
	{
		GotoState('stateHitBySpell');
	}

	return true;
}

// When the FireCrab is on his feet he is vulnerable to the Rictusempra spell
function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{

	local vector v;

	Super.HandleSpellRictusempra( spell, vHitLocation );

	if ( !IsInState('stayFlipped') )
	{
		GotoState('stateHitBySpell');
	}

	return true;
}


function Tick(float DeltaTime)
{

	Super.Tick(DeltaTime);

	//See if we should shoot at Harry
	if ( !IsInState('attackHarry') && !IsInState('Throwing') && !IsInState('CutIdle') &&
	 !IsInState('stateHitBySpell') && !IsInState('DoFlip')  && !IsInState('strafeAround') && !IsInState('StayFlipped'))
	{

		TimeUntilNextFire -= DeltaTime;

		if( TimeUntilNextFire < 0 )
		{
			if( VSize(playerHarry.Location - Location) < fAttackRange  &&  PlayerCanSeeMe() )
			{
				TimeUntilNextFire = TimeUntilNextFireDefault;
		
				GotoState('attackHarry');
			}
			else
			{
				TimeUntilNextFire = 2.0;
			}
		}
	}
	
}

// Play the firecrab roar
function playRoarSound()
{
	local int randNum;

	randNum = rand(4);

	switch (randNum)
	{
	case 0:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_roar';
		break;
	case 1:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_roar_A';
		break;
	case 2:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_roar_B';
		break;
	case 3:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_roar_C';
		break;
	}

	PlaySound( RoarSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );
}


//*************************************************************************************************************************
state stateHitBySpell
{
	function BeginState()
	{
		// Stop the footstep sound
		AmbientSound = None;
	}

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage(Name $ " : State stateHitBySpell ");

	if( --iNumSpellHitsToFlip <= 0 )
	{

		if( AnimSequence == 'flip2back' || AnimSequence == 'onback' )
		{
			fTimeOnBack += 2;
		}
		else
		{
			acceleration = vect(0,0,0);
			velocity = vect(0,0,0);
		}

		gotoState('DoFlip');
	}
	else
	{
		Velocity = vect(0,0,0);
		acceleration = vect(0,0,0);

		PlayAnim('KnockBack');
		FinishAnim();
	
		PlayAnim('look');
		FinishAnim();

		TimeUntilNextFire = TimeUntilNextFire + 1.0f;

		// Start the walking sound again
		AmbientSound = WalkingSound;

		GotoState('patrol');
	}
}

//*************************************************************************************************************************
state DoFlip
{

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		if ( vSize(velocity) <= 5 )
		{
			if ( OnALedge(location) )
			{
				vPush = pushDirection();
				SetCollisionSize(CollisionRadius-10,CollisionHeight);
				gotoState('FallOverLedge');
			}
		}
	}

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( Name $ " : State Do Flip ");

	fHighestZ = location.z;

	eVulnerableToSpell = SPELL_Flipendo;

	//PlaySound( sound'HPSounds.Critters_sfx.firecrab_hit', SLOT_none );
	playHitSound();

	loopAnim('flip2back');
	FinishAnim();
	
	loopAnim('onBack');
	sleep( fTimeOnBack );

	loopAnim('recover');
	sleep(1.0);
	//FinishAnim();

	// now that he's up he is vulnerable to Rictusempra again
	eVulnerableToSpell = SPELL_Rictusempra;

	FinishAnim();

	loopAnim('look');
	FinishAnim();

	loopAnim('idle');

	TimeUntilNextFire = TimeUntilNextFire + 1.0f;

	// Start the walking sound again
	AmbientSound = WalkingSound;

	fTimeOnBack = fTimeSpentOnBack;

	GotoState('patrol');

}



//*************************************************************************************************************************
state attackHarry
{
	
	function Tick(float DeltaTime)
	{
	}

  begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": attackHarry" );

	loopAnim('idle');
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	AmbientSound = None;

	if ( bPlayRoar == true )
	{
		TurnTo(playerHarry.location);

		playRoarSound();

		PlayAnim('roar');
		FinishAnim();
	}


wait:
	TurnTo(Location + Location-playerHarry.Location);

	// stop to fire
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	// Face away from Harry in order to fire
	TurnTo( Location + (Location - playerHarry.Location) );

	Sleep( 0.05 );
	gotostate('throwing');
goto 'wait';


}


//*************************************************************************************************************************
state throwing
{

	function BeginState()
	{
		iNumShots = iNumShotsBetweenPreAttack;
	}

	function Tick(float DeltaTime)
	{
	}

	function touch(actor other)
	{

		if( other.bBlockActors )
			HitWall( normal(Location - other.Location), other );
	}

	function HitWall(vector HitNormal, actor Wall)
	{
		SetLocation( OldLocation );

		// Set the velocity to reflect the normal
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );
	}

	begin:


	// Play the preattack sfx
	PlaySound( sound'HPSounds.Critters_sfx.firecrab_preAttack', SLOT_none );

	// Play the preattack anim
	PlayAnim('preattack');
	FinishAnim();

	while( LineOfSightTo(playerHarry) && iNumShots > 0)
	{
		iNumShots--;

		TurnTo( Location + (Location - playerHarry.Location) );

		target=playerHarry;

		//You dont see the crab rotate 180, but it makes the projectile come out his ass...
		SetRotation( Rotation + rot(0,32768,0) );

		smallSpell = spellFireSmall(SpawnSpell(class'spellFireSmall',playerHarry));
		smallSpell.iDamage = SpellDamage;
		smallSpell.fIncreaseHitTimeDistance = fIncreaseHitTimeDistance;
		smallSpell.fHitTimeIncrement = fHitTimeIncrement;
		smallSpell.iAccuracyMin = iAccuracyMin;
		smallSpell.iAccuracyMax = iAccuracyMax;

		SetRotation( Rotation + rot(0,32768,0) );

		PlaySound( AttackSound, SLOT_none );

		PlayAnim('attack');
		FinishAnim();

		// Look for Harry
		if ( !LineOfSightTo(playerHarry) && bMoveAround == true )
		{
			TurnTo( Location + (Location - playerHarry.Location) );

			vTemp = normal(playerHarry.location - location);
			vTemp2 = location cross playerHarry.location;
			if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("cross = "$vTemp2);
			if ( vTemp2.z > 0 )
			{
				loopAnim('strafeleft');
				vTemp2 = -(vTemp cross vec(0,0,1));
				if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("strafing left");
			}
			else if ( vTemp2.z < 0 )
			{
				loopAnim('straferight');
				vTemp2 = vTemp cross vec(0,0,1);
				if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("strafing right");
			}
			else
			{
				loopAnim('Walk');
				vTemp2 =  normal( location - playerHarry.location );
				if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("backing up");
			}
			
			acceleration = vTemp2;
			velocity = groundSpeed * vTemp2;

			sleep(1.5);
		}

		sleep(timeBetweenShots);

	}

	if ( iNumShots <= 0 )
	{
		gotoState('strafeAround');
	}
	else
	{

		// AFTER ATTACK

		// Wait a bit so he doesn't turn away while the spell is still firing. 
		sleep(0.8);

		TurnTo( Location + vMoveDir );

		//gotostate('attackHarry');

		vMoveDirRot = Rotation;
		vMoveDir = vector( vMoveDirRot );

		//Turn back towards the PatrolPoint you're currently moving to
		TurnTo( navP.Location );		

		// start the walking sound
		AmbientSound = WalkingSound;

		GotoState('patrol');
	}

}

state strafeAround
{

	function BeginState()
	{
		normalSpeed = groundSpeed;
		groundSpeed = strafeSpeed;
	}

	function EndState()
	{
		groundSpeed = normalSpeed;
	}

	begin:

	if ( bCanStrafe == true )
	{

		TurnTo( Location + (Location - playerHarry.Location) );

		vTemp = normal(playerHarry.location - location);
		vTemp2 = location cross playerHarry.location;


		if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("cross = "$vTemp2);
		if ( strafeLeft == true )
		{
			if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("strafing left");

			strafeLeft = false;

			vTemp2 = -(vTemp cross vec(0,0,1));
			rotationChange = rotator(vTemp2);
			rotationChange.yaw -= 4000;
			vTemp2 = vector(rotationChange);

			loopAnim('strafeleft',2.0);

		}
		else if ( strafeLeft == false )
		{
			if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("strafing right");

			strafeLeft = true;

			vTemp2 = vTemp cross vec(0,0,1);
			rotationChange = rotator(vTemp2);
			rotationChange.yaw += 4000;
			vTemp2 = vector(rotationChange);

			loopAnim('straferight',2.0);

		}
		else
		{
			gotoState('throwing');
		}

		acceleration = vTemp2;
		velocity = groundSpeed * vTemp2;

		sleep(0.65);

		// stop
		velocity = vect(0,0,0);
		acceleration = vect(0,0,0);

		TurnTo( Location + (Location - playerHarry.Location) );
	}

	// check if you should keep attacking Harry. If not return to patrol
	if( VSize(playerHarry.Location - Location) < fAttackRange )
	{
		TurnTo( Location + (Location - playerHarry.Location) );
		gotoState('throwing');
	}
	else
	{
		sleep(0.3);

		TurnTo( Location + vMoveDir );

		vMoveDirRot = Rotation;
		vMoveDir = vector( vMoveDirRot );

		//Turn back towards the PatrolPoint you're currently moving to
		TurnTo( navP.Location );		

		// start the walking sound
		AmbientSound = WalkingSound;

		GotoState('patrol');
	}

}

defaultproperties
{
     bMoveAround=True
     NormalSpeed=100
     strafeSpeed=150
     timeBetweenShots=0.2
     fIncreaseHitTimeDistance=200
     fHitTimeIncrement=0.5
     iAccuracyMax=10
     bPlayRoar=True
     SpellDamage=5
     iNumShotsBetweenPreAttack=2
     bCanStrafe=True
     iNumSpellHitsToFlipDefault=1
     bFlipPushable=True
     soundFalling(0)=Sound'HPSounds.Critters_sfx.firecrab_falling'
     soundFalling(1)=Sound'HPSounds.Critters_sfx.firecrab_falling_A'
     lockSpell=True
     MultiSkins(1)=WetTexture'HPParticle.hp_fx.General.GemWet'
     CollisionRadius=30
}
