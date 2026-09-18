// SpiderLarge: The large spiders that shoot webs at Harry
//				and attack him. 

class SpiderLarge expands Spider;

var vector vDir;
var vector vTemp;

var rotator jumpRotation;
var int iterationCheck;
var() float timeStunnedWhenHit;
var float savedCollision;
var() bool bDoEvent;



var (VisualFX)ParticleFX		fxDestroy1ParticleEffect;
var (VisualFX)ParticleFX		fxDestroy2ParticleEffect;

var Aragog spider;	// just for Aragog level


//************ Generic Functions ************************************************************
//*******************************************************************************************


function AddWebs()
{
//	iNumWebs++;
}

function SubWebs()
{
//	iNumWebs--;
}

function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellRictusempra( spell, vHitLocation );

playerHarry.clientMessage(self.name$ " : In state HandleSpellRictusempra");


	if ( !IsInState('OutForTheCount') )
	{
		groundSpeed = normalSpeed;

		gotoState('HitBySpell');
		return true;
	}

	return false; // we have an invalid hit
}

function Landed(vector HitNormal)
{
	local rotator landedRotation;

	Super.Landed(HitNormal);

	landedRotation = rotation;
	landedRotation.pitch = 0;

	SetRotation(landedRotation);
}

function unTouch(actor other)
{
	Super.unTouch(other);

	if ( !IsInState('OutForTheCount') )
	{
		if ( Other.IsA('SpiderMarker' ) )
		{
			// Check if it's this spiders' centerMarker
			if ( SpiderMarker(other) == currentMarker )
			{
				// stop
				Velocity = vect(0,0,0);
				Acceleration = vect(0,0,0);

				atTheEdge = true;

				groundSpeed = normalSpeed;

				// wait a random time at this spot
				gotoState('randomWait');
			}
		}
	}
}

// Returns true if the spider is in a position to attack harry.
function bool ReadyPosition()
{
	if ( vSize(playerHarry.location - location) < savedCollision + playerHarry.collisionRadius - (9 * drawScale) ) 
		return true;

	return false;
}


//************ States ************************************************************
//*******************************************************************************************
state preAttackCheck
{

	begin:

	bAttacking = true;
	groundSpeed = AttackSpeed;
	savedCollision = collisionRadius;


	if ( vSize2D(playerHarry.location - location) > jumpingDistanceFromHarry )
	{	
		gotoState('playPreAttackAnim');
	}
	else
	{
		gotoState('AttackHarry');
	}
}

state playPreAttackAnim
{

	begin:

	// stop moving
	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);

	// face Harry
	vTemp = vec(playerHarry.location.x, playerHarry.location.y, location.z);
	vDir  = normal(vTemp - location);
	desiredRotation = rotator(vDir);

	switch (ePreAttackAnim)
	{
		case ATTACK_NONE:
			sleep(0.5);
			break;
		case ATTACK_JUMP:
			PlayAnim('walk2jump');
			FinishAnim();
			PlaySound( sound'HPSounds.Critters_sfx.SPI_large_preattack', SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]200000, [Pitch]RandRange(3.5, 4.4),, false );
			loopAnim('jump');
			sleep(0.3);
			PlayAnim('jump2walk');
			FinishAnim();
			break;
		case ATTACK_REAR:
			if ( rand(2) == 0 )
			{
				PlaySound( sound'HPSounds.Critters_sfx.SPI_large_Hiss1', SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]200000, [Pitch]RandRange(0.8, 1.2),, false );
			}
			else
			{
				PlaySound( sound'HPSounds.Critters_sfx.SPI_large_Hiss2', SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]200000, [Pitch]RandRange(0.8, 1.2),, false );
			}
			PlayAnim('webAttack');
			FinishAnim();
			sleep(0.4);
			break;
	}

	gotoState('AttackHarry');



}

/*
state jumpingToHarry
{

	function Tick(float DeltaTime)
	{ 
		Super.Tick(DeltaTime);

		if ( vSize2D(playerHarry.location - location) <= jumpingDistanceFromHarry )
		{
			gotoState('stopJumping');
		}

		groundSpeed = 200;
		// turn toward harry
		vDir = normal(playerHarry.location - location);
		acceleration = vDir * (200);

		DesiredRotation = rotator(normal(playerHarry.location-location));
		SetRotation( rotator(normal(playerHarry.location-location)) );
	}

	begin:

	playAnim('walk2jump',2.5);
	finishAnim();

	loopAnim('jump',2.0);

	
}

state stopJumping
{
	begin:	

	groundSpeed = attackSpeed;

	playAnim('jump2walk',,0.2);
	finishAnim();

	gotoState('AttackHarry');

}
*/

state AttackHarry
{
	
	function BeginState()
	{
		if ( drawScale >= 1.00f )
		{
			SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale-(14*DrawScale), Default.CollisionHeight*DrawScale/Default.DrawScale);
		}
		else
		{
			SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale-(17), Default.CollisionHeight*DrawScale/Default.DrawScale);
		}

		iterationCheck = 0;
	}

	function Tick(float DeltaTime)
	{	
		Global.Tick(DeltaTime);

		// Check if the spider is in a position to bite Harry
		if ( ReadyPosition() ==  true   &&
			(baseHud(playerharry.myHud).bCutSceneMode == false)  )
		{
			gotoState('stateBiteHarry');
		}

	}
/*
	function unTouch(actor other)
	{
		if ( Other.IsA('SpiderMarker' ) )
		{
			// Check if it's this spiders' centerMarker
			if ( SpiderMarker(other) == currentMarker )
			{
				// stop
				Velocity = vect(0,0,0);
				Acceleration = vect(0,0,0);

				// wait a random time at this spot
				gotoState('randomWait');
	
			}
		}
	}
*/
	begin:

	loopAnim('walk',1.0);

	// If you are attacking then you are vulnerable to a spell
	eVulnerableToSpell=SPELL_Rictusempra;

	// turn toward harry
//	vDir = normal(playerHarry.location - location);
//	acceleration = vDir * AttackSpeed;

loop:

	MoveToward(playerHarry);
	
	sleep(0.1);

goto 'loop';
	
}

state stateBiteHarry
{
	function BeginState()
	{
//		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale-10, Default.CollisionHeight*DrawScale/Default.DrawScale);
	}

	function EndState()
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);

		// Done attacking this time. Go back to wander and look again			
		SetCollision(true,true,true);

		// attack done
		bAttacking = false;
	}

	begin:

//	playAnim('lungeAttackStart',2.0);
//	finishAnim();

	PlaySound( sound'HPSounds.Critters_sfx.SPI_large_jump', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]20000, [Pitch]RandRange(0.8, 1.2),, false );

	PlayAnim('lungeAttack',2.0);

	sleep(0.06);

	if ( rand(2) == 0 )
	{
		PlaySound( sound'HPSounds.Critters_sfx.SPI_large_bite1', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]250, [Pitch]RandRange(0.8, 1.2),, false );
	}
	else
	{
		PlaySound( sound'HPSounds.Critters_sfx.SPI_large_bite2', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]250, [Pitch]RandRange(0.8, 1.2),, false );
	}
 
	//if ( vSize(playerHarry.location - location) < collisionRadius + playerHarry.collisionRadius )
	if ( vSize(playerHarry.location - location) < savedCollision + playerHarry.collisionRadius )
		playerHarry.TakeDamage( fDamageAmount, instigator, vect(0,0,0), vect(0,0,0), '' );

	velocity = ( normal(location - playerHarry.location) * groundSpeed );
	acceleration = ( normal(location - playerHarry.location) * groundSpeed*2 );

	playAnim('lungeAttackEnd',1.3);
	finishAnim();

	sleep(0.2);

	gotoState('wander');

}


state HitBySpell
{
	function BeginState()
	{
		numSpells--;
	}

	begin:

	PlaySound( sound'HPSounds.Critters_sfx.SPI_hit', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]2000, [Pitch]RandRange(0.8, 1.2),, false );
	if ( rand(2) == 0 )
	{
		PlaySound( sound'HPSounds.Critters_sfx.SPI_large_ouch1', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]2000, [Pitch]RandRange(0.8, 1.2),, false );
	}
	else
	{
		PlaySound( sound'HPSounds.Critters_sfx.SPI_large_ouch2', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]2000, [Pitch]RandRange(0.8, 1.2),, false );
	}

	if ( numSpells > 0 )
	{
		// only stunned
	//	velocity = -(velocity);
	//	Acceleration = -(acceleration);

		velocity = vect(0,0,0);
		Acceleration = vect(0,0,0);

		playAnim('jump2walk',2.5);
		finishAnim();

		loopAnim('idle');
		sleep(timeStunnedWhenHit);
		
		gotoState('wander');
	}
	else
	{
		// flip over and twitch (out forever)
		gotoState('OutForTheCount');
	}

}

state OutForTheCount
{


	begin:

	eVulnerableToSpell=SPELL_none;

	if ( bDoEvent == true )
	{
		TriggerEvent( 'OutForTheCount', self, None );
	}

	// stop moving
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	PlaySound( sound'HPSounds.Critters_sfx.SPI_large_LandOnBack', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]200000, [Pitch]RandRange(0.8, 1.2),, false );

	PlayAnim('FlippedOver',1.4);
	FinishAnim();

	loopAnim('IdleOnBack');

	SetCollisionSize(15, 20);


	// There is a special case for the Aragog level. Once the spiders are out then 
	// they need to be destroyed so they don't clutter up the level. Do this here
	foreach AllActors( class'Aragog', spider )
		break;

	if ( spider != None )
	{
		// There is an Aragog in this level
		sleep(0.2);
		fxDestroy1ParticleEffect = spawn( class'HPParticle.WebFx',,,Location );
		fxDestroy2ParticleEffect = spawn( class'HPParticle.WebDust',,,Location );
		sleep(0.1);
		fxDestroy1ParticleEffect.ShutDown();
		fxDestroy2ParticleEffect.ShutDown();
		sleep(0.05);
		Destroy();
	}

 
}





// Default Props for the Spider
//*************************************************************************************************************************
defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skSpiderLargeMesh'
	DrawType=DT_Mesh;
	Physics=PHYS_Walking
	DrawScale=1;
	Menuname="SpiderLarge"
	GroundSpeed=100;
	AirSpeed=200;
	AirControl=2.0;
	AccelRate=4000;
 //   Mass=60;
	BaseEyeHeight=30;
	EyeHeight=30;
    Buoyancy=118.800003;
	RotationRate=5000;
	ambientglow=200; 
	bRotateToDesired=True
	CollisionRadius=40
	CollisionHeight=30
	RotationRate=(Pitch=100000,Yaw=100000,Roll=100000)

	IdleAnimName="walk" 
	bCollideWorld=true
	bcollideactors=true
	bProjTarget=true
	eVulnerableToSpell=SPELL_Rictusempra

	MaxStepHeight=+010.000000 
	sightRadius=500    
 
	normalSpeed=85
	attackSpeed=95
	fDamageAmount=3
	bDespawnable=true
	timeStunnedWhenHit=1.0
	bDoEvent=False

}

