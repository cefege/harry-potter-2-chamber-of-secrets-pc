
// Class Name  : spellFireLarge
//
// Created on  : 04/18/2002
// Authored by : Janet Weddle
// 
// Description : The spellFire class implements the Fire spell specific code
//				 Harry can not cast Fire, instead the Firecrabs cast at Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

//class spellFireLarge extends spellFire;
class spellFireLarge extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables

var (VisualFX)ParticleFX		fxHeadParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHeadParticleEffectClass;

var (VisualFX)ParticleFX		fxFlyParticleEffect;			
var (VisualFX)class<ParticleFX>	fxFlyParticleEffectClass;

enum enumTargetZone
{
	ZONE_ONE,
	ZONE_TWO,
};

var enumTargetZone eTargetZone;

var float	fGravityEffect;
var vector	CurrentDir;	
var vector	savedVelocity;

var bool	bBounce;
var vector	currentVelocity;
var float   GlobalSpeed;

var float fIncreaseHitTimeDistance;
var float	fHitTimeIncrement;
var float timeToTarget;

var vector hitTarget;		// The location where the spell should hit
var float GrenadeRadius;
var float GrenadeBounceInterval;
var float GrenadeGravity;
var float GrenadeExplosionGravity;
var int iDamage;
var fireballLarge fireball; 
var float smallDamage;



// --------------------------------------------------------------------------------------------
// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	
	// Create our Head FX
	fxHeadParticleEffect = spawn( fxHeadParticleEffectClass );
	fxHeadParticleEffect.SetLocation( Location );
	fxHeadParticleEffect.SetRotation( fxHeadParticleEffect.default.rotation );

	// Create our Fly FX
	fxFlyParticleEffect = spawn( fxFlyParticleEffectClass );
	fxFlyParticleEffect.SetLocation( Location );
	fxFlyParticleEffect.SetRotation( fxFlyParticleEffectClass.default.rotation );

}

function timer()
{
	bBounce = false;

}

// Grid mover should not be pushed by spellFireLarge
function bool IsRelevantToMover()
{
	return false;
}

function OnSpellShutdown()
{
	// Clean up
	if( fxHeadParticleEffect != None )			
		fxHeadParticleEffect.Shutdown();

	if( fxFlyParticleEffect != None )
	{
		fxFlyParticleEffect.Shutdown();
	}

}


function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	// *****This spell is directed at Harry
	// *****Harry is not derived from HPawn so there is no HandleSpell function
	return false; // not a valid hit
}

function ShootFireballs()
{
	local int i;
	local crabFire fireball;
	local spellFireSmall smallFire;
	local int NumFireballs;
	local rotator rotate_fireball;
	local vector fireball_locn, harrys_head;

	harrys_head = playerHarry.location;

	harrys_head.z += playerHarry.collisionHeight/2;

	NumFireballs = 5;

	rotate_fireball = rotator(harrys_head - location);

	rotate_fireball.roll = 0;
	rotate_fireball.pitch += (65536*10) / 4;

	// Shake the camera when the fireballs explode
	playerHarry.ShakeView( 0.3, 200, 200 );

	for (i=0; i<NumFireballs; ++i)
	{
		rotate_fireball.yaw = (65536 / NumFireballs) * i + rand(10000);
		fireball_locn = location;
		smallFire = spawn(class'spellFireSmall',owner,,fireball_locn, rotate_fireball);
	}

}

function bounce( vector HitNormal)
{

playerHarry.clientMessage("In bounce.  Velocity :  " $velocity);

	// Get out of the wall or floor
	SetLocation( OldLocation );

	// Slow it down
	Velocity *= 0.40;

	// Set the velocity to reflect the normal
	Velocity = MirrorVectorByNormal( Velocity, HitNormal );

	// Set the rotation of the particleEffect. 
	fxFlyParticleEffect.SetRotation( rotator(velocity) );

	// Set our Direction vector
	CurrentDir = vector( Rotation );

	// Aim the Current Direction to the normal.
	CurrentDir += HitNormal;
	
	SetRotation( rotator( CurrentDir ) );

}


// ZONE_ONE : There is line of sight to Harry
// ZONE_TWO : There is no line of sight to Harry but the big firecrabs will fire anyway. Higher and longer
function SetTargetZone(int z)
{
	switch (z)
	{
	case 0:
		eTargetZone = ZONE_ONE;
//		playerHarry.clientMessage("ZONE_ONE");
		break;
	case 1:
		eTargetZone = ZONE_TWO;
//		playerHarry.clientMessage("ZONE_TWO");
		break;
	default:
		eTargetZone = ZONE_ONE;
//		playerHarry.clientMessage("DEFAULT : ZONE_ONE");
		break;
	}
}

function float SetAngle()
{
	local float speed;
	local float gravity;
	local float distance;
	local float angle;
	local rotator BehindHarry;
	local vector target;

	BehindHarry = rotator( location - playerHarry.location );

	target = playerHarry.location + (vector(BehindHarry) * (2*playerHarry.collisionRadius));

	distance = vSize(location-target);

	GlobalSpeed = 300;

	if ( distance > 550 )
	{
		gravity = 300;
	}
	else if ( distance > 475 )
	{
		gravity = 200;
	}
	else
	{
		gravity = 0;
	}

	speed = GlobalSpeed;   

	angle = ( ( Sin( distance * gravity / (speed*speed) ) ) ) / 2;

//	playerHarry.clientMessage("Angle :  " $angle);
//	playerHarry.clientMessage("Distance : " $distance);

	return angle;	

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

	minYaw = rotationFromHarry.yaw - 4095;
	randomYaw = minYaw + rand(12288);

	if ( rand(2) == 0 )
	{
		randomYaw = -randomYaw;
	}

	tempRot.yaw = rotationFromHarry.yaw + randomYaw;
	tempVector = vector(tempRot);
	
	newTarget = playerHarry.location + ( tempVector * (rand(100)+100) );

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

	SetTimer(0,false);
	OnSpellShutdown();		
	Destroy();

}

state auto StateIdle
{
	begin:
	GotoState('StateFlying');
}

state StateFlying
{

	function BeginState()
	{

//		fGravityEffect = Region.Zone.ZoneGravity.Z;
		fGravityEffect = GrenadeGravity;

		hitTarget = GetTarget();
		timeToTarget = GetTime();

		Velocity = ( hitTarget - location ) / timeToTarget;

		Velocity.z = ((hitTarget.z - location.z) - (0.5f * fGravityEffect * (timeToTarget*timeToTarget)) ) / timeToTarget;
	
		SetTimer(GrenadeBounceInterval,false);

	}

	function Landed(vector HitNormal)
	{
		gotoState('stateExplode');
	}
	
	function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
	{
		aHit.TakeDamage( iDamage, instigator, vect(0,0,0), vect(0,0,0), '' );
		return true; // we have a valid hit
	}
	
	function bool OnSpellHitWall( Actor aWall, vector HitNormal )
	{
		if ( bBounce == false  )
		{
			// If you are against a flat wall, continue to bounce
//			if ( HitNormal != vec(0,0,1) && HitNormal != vec(0,0,-1) )
//			{
//playerharry.clientMessage("Hit Wall: bBounce is false: On a wall");
//				bounce( HitNormal );
//			}
//			else
//			{
playerharry.clientMessage("Hit Wall: bBounce is false ");
				gotoState('stateExplode');

//				return false;
//			}
		}
		else
		{
playerharry.clientMessage("Hit Wall: bBounce is true");
			bounce( HitNormal );
		}
		return false;
	}


	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );

	
		if ( bBounce == false )
		{
			//velocity.z += -(fGravityEffect*5) * fTimeDelta;
			velocity.z += (fGravityEffect * fTimeDelta);
		}
		else
		{
			//velocity.z += -(fGravityEffect*5) * fTimeDelta;
			velocity.z += (fGravityEffect * fTimeDelta);
		}
			
		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );
		}

		// Update our head particles
		if( fxHeadParticleEffect != None )
		{
			fxHeadParticleEffect.SetLocation( location );
		}
	}
}

state stateExplode
{

	begin:

	fireball = spawn(class'fireballLarge',owner,,location, rotation);
	fireball.GrenadeRadius = GrenadeRadius;
	fireball.iDamage = iDamage;
	fireball.smallDamage = smallDamage;
	fireball.GrenadeExplosionGravity = GrenadeExplosionGravity;

}



// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{

	fxFlyParticleEffectClass=class'CrabFire'
	fxHeadParticleEffectClass=class'Crabfireball'
	fxHitParticleEffectClass=class'SmokeExplo_01'
	
	// --- Base Spell
	spellType=SPELL_Fire
	   
	// --- ParticleFX
//	Speed=300.0f
    DrawType=DT_None

//	fGravityEffect=200	// seems really high but I like the way it looks
	bBounce=True

	iDamage=10
	fIncreaseHitTimeDistance=200
	fHitTimeIncrement=0.5

}

// --------------------------------------------------------------------------------------------
// spellFire.uc - End of file   
// --------------------------------------------------------------------------------------------