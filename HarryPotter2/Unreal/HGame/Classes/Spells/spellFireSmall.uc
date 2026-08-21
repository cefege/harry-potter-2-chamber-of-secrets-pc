
// Class Name  : spellFireSmall
//
// Created on  : 04/18/2002
// Authored by : Janet Weddle
// 
// Description : The spellFire class implements the Fire spell specific code
//				 Harry can not cast Fire, instead the Firecrabs cast at Harry
// 
// NOTE: 06/16/2002 Using a different function to find the corrent trajectory of the 
//					the spell. BUT...this does not work when the smallfirespell is spawned
//					from the largefirespell. Therefore, I had to split the class behavior
//					based on the owner. If the owner is a LargeFirecrab the spell will
//					go to stateExplosion otherwise it will go to stateFlying
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

//class spellFireSmall extends spellFire;
class spellFireSmall extends baseSpell;
 

// --------------------------------------------------------------------------------------------
// *** Variables

var float	fGravityEffect;
var vector	CurrentDir;	

var vector	currentVelocity;

var float GlobalSpeed;

var float fIncreaseHitTimeDistance;
var float fHitTimeIncrement;
var int iDamage;
var vector hitTarget;		// The location where the spell should hit

var int iAccuracyMin;
var int iAccuracyMax;

var float GrenadeExplosionGravity;

// --------------------------------------------------------------------------------------------
// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	
	// Create our flying FX
	// NOTE: You can let base spell do this by calling initSpell() when you spawn BUT..
	// by giving the firecrab as the one casting the spell baseSpell will decide that the
	// spell needs to come out of the front of the firecrab not his 'back'. 
	fxFlyParticleEffect = spawn( fxFlyParticleEffectClass );
	fxFlyParticleEffect.SetLocation( Location );
	fxFlyParticleEffect.SetRotation( fxFlyParticleEffect.default.rotation );

	// Init our seeking current Dir var
	CurrentDir = normal(vector(Rotation) + vec(0,0,0.4f) );
	SetRotation( rotator( CurrentDir ) );
}

function OnSpellShutdown()
{
}


// Grid mover should not be pushed by spellFireSmall
function bool IsRelevantToMover()
{
	return false;
}


function float getTime()
{
	local float t;
	local float distance; 

	distance = vSize(location - playerHarry.location);

	t = (distance / fIncreaseHitTimeDistance) * fHitTimeIncrement;

//	playerHarry.clientMessage("Distance to harry : " $distance);

	return t;

}


function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
{
	aHit.TakeDamage( iDamage, instigator, vect(0,0,0), vect(0,0,0), '' );
	return true; // we have a valid hit
}

function bool OnSpellHitHPawn( Actor aHit, vector HitLocation )
{
	spawn(class'FireballOnHarry',,,Location);
	return true; // we have a valid hit
}

function bool OnSpellHitWall( Actor aWall, vector HitNormal )
{
	spawn(class'fireball',,,Location);
	return true; // we have a valid hit
}



function float SetAngle()
{
	local float speed;
	local float gravity;
	local float distance;
	local float angle;
	local rotator BehindHarry;
	local vector target;

	// the rotation vector that points behind Harry, away from the firecrab
	BehindHarry = rotator( location - playerHarry.location );

	target = playerHarry.location + (vector(BehindHarry) * (2*playerHarry.collisionRadius));

	distance = vSize(location-target);
 
	GlobalSpeed = 300;

	if ( distance > 390. )
	{
		gravity = 350;
		GlobalSpeed = 350;
	}
	else if ( distance > 356 )
	{
		gravity = 300;
	}
	else
	{
		gravity = 200;
	}

	speed = GlobalSpeed;

//	playerHarry.clientMessage("Distance : " $distance);

	angle = ( ( Sin( distance * gravity / (speed*speed) ) ) ) / 2;

	return angle;	

}

function vector GetTarget()
{
	local vector directionFromHarry;
	local rotator rotationFromHarry;
	local int randomYaw, minYaw;
	local rotator tempRot;
	local vector tempVector;
	local vector newTarget;

	directionFromHarry = normal(owner.location - playerHarry.location);
	rotationFromHarry = rotator(directionFromHarry);

	randomYaw = rand(65536);

	tempRot.yaw = randomYaw;
	tempVector = vector(tempRot);

//	tempRot.yaw = rotationFromHarry.yaw + randomYaw;
//	tempVector = vector(tempRot);
	
	newTarget = playerHarry.location + ( tempVector * (rand(iAccuracyMax)+iAccuracyMin) );

	return newTarget;
}


function PlayerCutCapture()
{
	gotoState('CutIdle');
}


// --------------------------------------------------------------------------------------------
// *** States

state CutIdle
{

	begin:

	OnSpellShutdown();		
	Destroy();

}

state auto StateIdle
{
	begin:

	if (owner.IsA('FirecrabLarge') )
	{
		gotoState('StateExplosion');
	}
	else
	{
		GotoState('StateFlying');
	}
}

state StateFlying
{

	function BeginState()
	{
		SetPhysics(PHYS_Falling);

		Velocity = ComputeTrajectoryByTime( location, GetTarget(), getTime() );

	}

/*	function ProcessTouch(Actor Other, vector HitLocation)
	{
		if( pawn(other) == instigator )
			return;
	
		if( other.IsA('Harry') )
		{
			other.TakeDamage( fDamageAmount, instigator, vect(0,0,0), vect(0,0,0), '' );
			CreateHitEffects( Other, HitLocation );
		}
		else
		{
			spawn(class'fireball',,,Location);
		}
	
		OnSpellShutdown();
		
		Destroy();
		
	}

	function HitWall(vector HitNormal, actor HitWall)
	{
		spawn(class'fireball',,,Location);
		Super.HitWall( HitNormal, HitWall );
	}
*/
	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
			
		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );
		}
	}
}


state StateExplosion
{

	function BeginState()
	{
		// Set our Direction vector
		CurrentDir = vector( Rotation );

		fGravityEffect = GrenadeExplosionGravity;
		CurrentDir.z = SetAngle();
		Speed = GlobalSpeed;

		SetRotation( rotator( CurrentDir ) );

		Velocity	 = vector(Rotation) * Speed;
		currentVelocity = Velocity;

	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );

	//	velocity.z += -fGravityEffect * fTimeDelta;
		velocity.z += fGravityEffect * fTimeDelta;
			
		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );
		}
	}

	begin:
		loop:
		sleep(1);
		goto 'loop';
}





// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{

	fxFlyParticleEffectClass=class'CrabFire'	
	
	// --- Base Spell
	spellType=SPELL_Fire
	
	fxHitParticleEffectClass=class'SmokeExplo_01'
//	fxReactParticleEffectClass=class'HPParticle.Crabfire2'
	
	// --- ParticleFX
	Speed=300.0f
    DrawType=DT_None

	fIncreaseHitTimeDistance=200
	fHitTimeIncrement=0.5
	iDamage=5

}

// --------------------------------------------------------------------------------------------
// spellFireSmall.uc - End of file   
// --------------------------------------------------------------------------------------------