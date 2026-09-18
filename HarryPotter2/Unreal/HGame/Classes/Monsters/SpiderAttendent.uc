// Class Name  : SpiderAttendent
//
// Created on  : 07/07/2002
// Authored by : Janet Weddle
// 
// Description : 
//
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpiderAttendent expands Spider;

var vector vDir;
var vector vTemp;
var rotator vRot;

var rotator jumpRotation;
var() float timeStunnedWhenHit;
var float savedCollision;

var (VisualFX)ParticleFX		fxDestroy1ParticleEffect;
var (VisualFX)ParticleFX		fxDestroy2ParticleEffect;

var bool bMoveTowardHarry;
var float menacingTime;
var float moveToHarryTime;

//var Aragog spider;	// just for Aragog level


//************ Generic Functions ************************************************************
//*******************************************************************************************


function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellRictusempra( spell, vHitLocation );

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

	if ( !IsInState('OutForTheCount') )
	{
		gotoState('preAttackCheck');
	}

}

function bool PawnCantStandOnMe()
{
	return false;
}

// Returns true if the spider is in a position to attack harry.
function bool ReadyPosition()
{
	if ( vSize(playerHarry.location - location) < savedCollision + playerHarry.collisionRadius - (9 * drawScale) ) 
		return true;

	return false;
}

function Timer()
{
	SetCollision( [NewColActors]true, [NewBlockActors]true);
}


//************ States ************************************************************
//*******************************************************************************************
state preAttackSetup
{
	begin:

	SetTimer(2.0, false);

	if ( DrawScale != Default.Drawscale )
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);
	}
	savedCollision = collisionRadius;
	numSpells = numSpellsDefault;

	if ( location.z <= playerHarry.location.z + playerHarry.eyeHeight )
	{
		loopAnim('Walk');
		MoveToward(playerHarry);
		gotoState('preAttackCheck');
	}
	else
	{
		acceleration = vect(0,0,0);
		velocity = vect(0,0,0);
	}
}


state preAttackCheck
{

	begin:

	bAttacking = true;
	groundSpeed = AttackSpeed;

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

		moveToHarryTime = rand(5) + 6;
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

		moveToHarryTime -= DeltaTime;

		if ( moveToHarryTime <= 0 )
		{
			gotoState('stateBeMenacing');
		}
	}

	begin:

	loopAnim('walk',1.0);

	// If you are attacking then you are vulnerable to a spell
	eVulnerableToSpell=SPELL_Rictusempra;

	// turn toward harry
	vDir = normal(playerHarry.location - location);
	acceleration = vDir * AttackSpeed;

loop:

	MoveToward(playerHarry);
	
	sleep(0.1);

goto 'loop';
	
}

state stateBeMenacing
{
	function BeginState()
	{
		menacingTime = rand(3) + 1;
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

		menacingTime -= DeltaTime;

		// Stop being menacing
		if ( menacingTime <= 0 )
		{
			gotoState('preAttackCheck');
		}

		// turn toward Harry.
		vDir = normal(playerHarry.location - location);
		vRot = rotator(playerHarry.location - location);

		DesiredRotation = vRot;
	}

	begin:

	loopAnim('idle');
			
	// Stop moving
	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);

}

state stateBiteHarry
{

	function EndState()
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);

		// Done attacking this time.			
		SetCollision(true,true,true);

		// attack done
		bAttacking = false;
	}

	begin:

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
 
	if ( vSize(playerHarry.location - location) < default.collisionRadius + playerHarry.collisionRadius )
		playerHarry.TakeDamage( fDamageAmount, instigator, vect(0,0,0), vect(0,0,0), '' );

	velocity = ( normal(location - playerHarry.location) * groundSpeed );
	acceleration = ( normal(location - playerHarry.location) * groundSpeed*2 );

	playAnim('lungeAttackEnd',1.3);
	finishAnim();

	sleep(0.2);

	// these spiders never wander. they only attack Harry. 
	gotoState('preAttackCheck');

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
		
		gotoState('preAttackCheck');
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

	TriggerEvent( 'OutForTheCount', self, None );

	// stop moving
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	PlaySound( sound'HPSounds.Critters_sfx.SPI_large_LandOnBack', SLOT_None, [Volume]RandRange(0.9, 1.0), [Radius]200000, [Pitch]RandRange(0.8, 1.2),, false );

	PlayAnim('FlippedOver',1.4);
	FinishAnim();

	loopAnim('IdleOnBack');

	SetCollisionSize(playerHarry.CollisionRadius, Default.CollisionHeight);


	// There is a special case for the Aragog level. Once the spiders are out then 
	// they need to be destroyed so they don't clutter up the level. Do this here

	// These spiders are ONLY in the Aragog level 

	sleep(0.2);
	fxDestroy1ParticleEffect = spawn( class'HPParticle.WebFx',,,Location );
	fxDestroy2ParticleEffect = spawn( class'HPParticle.WebDust',,,Location );
	sleep(0.1);
	fxDestroy1ParticleEffect.ShutDown();
	fxDestroy2ParticleEffect.ShutDown();
	sleep(0.05);
	Destroy();
 
}





// Default Props for the Spider
//*************************************************************************************************************************
defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skSpiderLargeMesh'
	DrawType=DT_Mesh;
	Physics=PHYS_Walking
	DrawScale=1;
	Menuname="SpiderAttendent"
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
	sightRadius=2500    
 
	normalSpeed=95
	attackSpeed=95
	fDamageAmount=3
	bDespawnable=true
	timeStunnedWhenHit=1.0

	bBlockActors=False
	bCollideActors=False
	
}

