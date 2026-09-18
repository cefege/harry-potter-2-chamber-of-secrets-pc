//===============================================================================
//  firecrabLarge
//===============================================================================


class firecrabLarge extends firecrab;


// *** Variables

var spellFireLarge largeSpell;
var spellFireSmall smallSpell;
var int counter;
var vector spellLocation;
var vector tempVector;
var rotator tempRotator;
var vector spellOrigin;
var int SpellSpraySpreadAmount;//The angle inbetween each of the spray spells.

var() float smallSpellDamage;
var() float largeSpellDamage;
var() float GrenadeRadius;
var() float timeBetweenShots;

var() float fLargeIncreaseHitTimeDistance;
var() float	fLargeHitTimeIncrement;

var() float fSmallIncreaseHitTimeDistance;
var() float	fSmallHitTimeIncrement;

var() int iAccuracyMin;
var() int iAccuracyMax;

var() bool bPlayRoar;

var() float GrenadeBounceInterval;
var() float GrenadeGravity;
var() float GrenadeOnlyDistance;
var() float GrenadeExplosionGravity;


// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


//** Functions 

// When the FireCrab is on his back he is vulnerable to the Flipendo spell
function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellFlipendo( spell, vHitLocation );

	// Call HandleSpellRictusempra so we only have to update one set of code. 
	GotoState('stateHitBySpell');
	
	return true;
}

// When the FireCrab is on his feet he is vulnerable to the Rictusempra spell
function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{

	local vector v;

	Super.HandleSpellRictusempra( spell, vHitLocation );

	if ( !IsInState('stayFlipped') )
	{
		if( --iNumSpellHitsToFlip <= 0 )
		{
			GotoState('stateHitBySpell');
		}
	}

	return true;
}

function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	if ( !IsInState('attackHarry') && !IsInState('Throwing') &&
		 !IsInState('stateHitBySpell') && !IsInState('DoFlip')  && !IsInState('stayFlipped') )
	{

		//See if we should shoot at Harry
		TimeUntilNextFire -= DeltaTime;

		if( TimeUntilNextFire < 0 )
		{
			if( VSize(playerHarry.Location - Location) < fAttackRange )
			{
				TimeUntilNextFire = TimeUntilNextFireDefault;

				GotoState('attackHarry');	
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
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_large_roar';
		break;
	case 1:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_large_roar_A';
		break;
	case 2:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_large_roar_B';
		break;
	case 3:
		RoarSound = sound'HPSounds.Critters_sfx.firecrab_large_roar_C';
		break;
	}

	PlaySound( RoarSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );


}


//*************************************************************************************************************************
state stateHitBySpell
{

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage(Name $ " : State stateHitBySpell ");

playerHarry.clientMessage("Velocity: " $velocity);

	// Stop the footstep sound
	AmbientSound = None;

	fHighestZ = location.z;

//	if( AnimSequence == 'flip2back' || AnimSequence == 'onback' )
//	{
//		desiredRotation.Yaw = Rotation.Yaw + 34400	;

//		acceleration = vect(0,0,0);
//		velocity = vect(0,0,0);

//		gotoState('stayFlipped');
//	}
//	else
//	{
		GotoState('DoFlip');
//	}
}

//*************************************************************************************************************************
state DoFlip
{
	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( Name $ " : State Do Flip ");

	eVulnerableToSpell = SPELL_Flipendo;

	RotationRate.yaw = 50000;

	//PlaySound( sound'HPSounds.Critters_sfx.firecrab_hit', SLOT_none );
	playHitSound();

	loopAnim('flip2back');
	FinishAnim();
	
	GotoState('stayflipped');

}


//*************************************************************************************************************************
state attackHarry
{
	
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

	gotostate('throwing');

}


//*************************************************************************************************************************
state throwing
{

	function BeginState()
	{
		// set spell location to vec(0,0,0) so we start fresh
		spellLocation = vect(0,0,0);
	}

	begin:

	// stop to fire
	velocity = vect(0,0,0);
	acceleration = vect(0,0,0);

	// Face away from Harry in order to fire
	TurnTo( Location + (Location - playerHarry.Location) );

	if ( Vsize(playerHarry.location-location) < GrenadeOnlyDistance)
	{

		// Play the preattack sfx
		PlaySound( sound'HPSounds.Critters_sfx.firecrab_preAttack', SLOT_none );

		// Play the preattack anim
		PlayAnim('preattack');
		FinishAnim();
	}

	for( counter = 0; counter < 4; counter++ )
	{

		if ( Vsize(playerHarry.location-location) < GrenadeOnlyDistance)
		{
			DesiredRotation.Yaw = rotator(location - playerHarry.Location).yaw;
	
			spellLocation = playerHarry.Location - vect(0,0,5) + VRand()*10;
			TempVector = spellLocation;

			spellOrigin = location + (-vector(rotation) * 3);
			spellOrigin = spellOrigin + vec(0,0,13);
			smallSpell = spawn(class'spellFireSmall',,,spellOrigin,rotation );
			smallSpell.iDamage = SmallSpellDamage;
			smallSpell.hitTarget = spellLocation;
			smallSpell.fIncreaseHitTimeDistance = fSmallIncreaseHitTimeDistance;
			smallSpell.fHitTimeIncrement = fSmallHitTimeIncrement;
			smallSpell.iAccuracyMin = iAccuracyMin;
			smallSpell.iAccuracyMax = iAccuracyMax;

			PlaySound( AttackSound, SLOT_none );

			PlayAnim('attack');
			FinishAnim();
		}

		Sleep(timeBetweenShots);
	}

	// Play the preattack sfx
	PlaySound( sound'HPSounds.Critters_sfx.firecrab_preAttack', SLOT_none );

	// Play the preattack anim
	PlayAnim('preattack');
	FinishAnim();

	//largeSpell = spellFireLarge( SpawnSpellEx(class'spellFireLarge',location+vec(0,0,12), rotation+rot(0,32768,0)) );
	largeSpell = spawn(class'spellFireLarge',self,,location+vec(0,0,12),rotation+rot(0,32768,0) );
	largeSpell.fIncreaseHitTimeDistance = fLargeIncreaseHitTimeDistance;
	largeSpell.fHitTimeIncrement = fLargeHitTimeIncrement;
	largeSpell.iDamage = largeSpellDamage;
	largeSpell.GrenadeRadius = GrenadeRadius;
	largeSpell.smallDamage = SmallSpellDamage;

	largeSpell.GrenadeBounceInterval = GrenadeBounceInterval;
	largeSpell.GrenadeGravity = GrenadeGravity;
	largeSpell.GrenadeExplosionGravity = GrenadeExplosionGravity;


	// AFTER FIRING

	// Wait a bit so he doesn't turn away while the spell is still firing. 
	sleep(0.8);

	TurnTo( Location + vMoveDir );

	vMoveDirRot = Rotation;
	vMoveDir = vector( vMoveDirRot );

	//Turn back towards the PatrolPoint you're currently moving to
	TurnTo( navP.Location );		

	// start the walking sound
	AmbientSound = WalkingSound;

	GotoState('patrol');

}

defaultproperties
{
     SpellSpraySpreadAmount=3000
     smallSpellDamage=5
     largeSpellDamage=10
     GrenadeRadius=200
     timeBetweenShots=0.7
     fLargeIncreaseHitTimeDistance=200
     fLargeHitTimeIncrement=0.5
     fSmallIncreaseHitTimeDistance=200
     fSmallHitTimeIncrement=0.5
     iAccuracyMin=50
     iAccuracyMax=100
     bPlayRoar=True
     GrenadeBounceInterval=2
     GrenadeGravity=-512
     GrenadeOnlyDistance=500
     GrenadeExplosionGravity=-350
     RoarSound=Sound'HPSounds.Critters_sfx.firecrab_large_roar'
     AttackSound=Sound'HPSounds.Critters_sfx.firecrab_large_attack'
     fAttackRange=1000
     iNumSpellHitsToFlipDefault=3
     bFlipPushable=True
     soundFalling(0)=Sound'HPSounds.Critters_sfx.firecrab_large_falling'
     soundFalling(1)=Sound'HPSounds.Critters_sfx.firecrab_large_falling_A'
     lockSpell=True
     SightRadius=1000
     PeripheralVision=0.05
     BaseEyeHeight=30
     EyeHeight=30
     DrawScale=4
     MultiSkins(1)=WetTexture'HPParticle.hp_fx.General.Gem2Wet'
     CollisionRadius=52
     CollisionHeight=44
     Mass=200
}
