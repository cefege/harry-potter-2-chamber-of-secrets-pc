
// Class Name  : spellWeb
//
// Created on  : 06/07/2002
// Authored by : Janet Weddle
// 
// Description : The spellWeb class implements the Web spell specific code
//				 Harry can not cast Web, instead the large spiders cast at Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

//class spellFireLarge extends spellFire;
class spellWeb extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables

var (VisualFX)ParticleFX		fxHeadParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHeadParticleEffectClass;

var float	fGravityEffect;
var vector	CurrentDir;	
var vector	savedVelocity;

var bool	bBounce;
var vector	currentVelocity;
var float   GlobalSpeed;
var float	fTimeToHitTarget;
var() float fIncreaseHitTimeDistance;
var() float	fHitTimeIncrement;

//var vector oldLocation;
var vector hitDirection;
var vector myHitLocation;

var float fWebLifetime;
var SpiderStickyWeb web;


// --------------------------------------------------------------------------------------------
// *** Constants

// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	
	// Create our Head FX
	fxHeadParticleEffect = spawn( fxHeadParticleEffectClass );
	fxHeadParticleEffect.SetLocation( Location );
	fxHeadParticleEffect.SetRotation( fxHeadParticleEffect.default.rotation );

}

function timer()
{

}

function OnSpellShutdown()
{
	// Clean up
	if( fxHeadParticleEffect != None )			
		fxHeadParticleEffect.Shutdown();
}

function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
{
	return true;
}

function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	// *****This spell is directed at Harry
	// *****Harry is not derived from HPawn so there is no HandleSpell function
	return false;
}


function bounce( vector HitNormal)
{

	// Get out of the wall or floor
	SetLocation( OldLocation );

	// Slow it down
	Velocity *= 0.70;

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

function vector getTarget()
{


	local rotator AroundHarry;
	local vector target;

	// get the rotator from the current location to harry
	AroundHarry = rotator( location - playerHarry.location  );

	// get a random rotation around Harry. If it happens to fall behind him it will bounce off of him.
	AroundHarry.Yaw = Rand(65534) + 1;

	target = playerHarry.location + (vector(AroundHarry) * (playerHarry.collisionRadius));


	return target;
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


function float SetAngle()
{
	local float speed;
	local float gravity;
	local float distance;
	local float angle;
	local rotator AroundHarry;
	local vector target;

	// get the rotator from the current location to harry
	AroundHarry = rotator( location - playerHarry.location  );

	// get a random rotation around Harry. If it happens to fall behind him it will bounce off of him.
	AroundHarry.Yaw = Rand(65534) + 1;

	target = playerHarry.location + (vector(AroundHarry) * (playerHarry.collisionRadius));

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


function PlayerCutCapture()
{
	gotoState('CutIdle');
}



function bool OnSpellHitWall( Actor aWall, vector HitNormal )
{
	bBounce = false;
	return true; // we have a valid hit
}

// If the web hits harry or another actor bounce it off of them
function ProcessTouch(Actor Other, vector HitLocation)
{
	if ( !other.IsA('SpiderMarker') && !other.IsA('LargeSpider') && !other.IsA('SpellWeb'))
	{
		myHitLocation = HitLocation;
		gotoState('stateBouncing');
	}
}

// The  has been changed to PHYS_Falling to use the new trajectory function. No longer hits walls
// now only gets landed
function Landed(vector HitNormal)
{
	bBounce = false;

	web = spawn(class'SpiderStickyWeb',owner,,OldLocation,rot(0,0,0));
	web.fWebLifetime = fWebLifetime;

	OnSpellShutdown();	
	Destroy();

}


/*	// The web will bounce until it hits a wall. We don't want it to explode on Harry or on other actors
	// (spiders or movers)
	function HitWall(vector HitNormal, actor HitWall)
	{

		bBounce = false;

		// Move back a step
//		SetLocation( OldLocation );

//		spawn(class'SpiderStickyWeb',,,OldLocation+vec(0,0,100),rotator(HitNormal));
		Super.HitWall( HitNormal, HitWall );

		OnSpellShutdown();	
		Destroy();

	}
*/	

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
//		playerHarry.clientMessage("Get Time returned : " $getTime());

		Velocity = ComputeTrajectoryByTime( location, playerHarry.location, getTime() );
//		Velocity = ComputeTrajectoryByTime( location, playerHarry.location, 2.0 );
	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
				
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


state stateBouncing
{
	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );

		velocity.z += (-fGravityEffect) * fTimeDelta;
				
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

	begin:

//	playerHarry.clientMessage("Enter state bouncing");

	// get the vector from the hit location to the previous location
	hitDirection = normal( OldLocation - myHitLocation );

	bounce(hitDirection);

}





// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{

	// --- ParticleFX
	fxFlyParticleEffectClass=class'CrabFire'
	fxHeadParticleEffectClass=class'Crabfireball'
	fxHitParticleEffectClass=class'SmokeExplo_01'
	
	// --- Base Spell
	spellType=SPELL_Web
	Physics=PHYS_Falling
	
    DrawType=DT_None
	bBounce=True
	fIncreaseHitTimeDistance=150
	fHitTimeIncrement=0.5

	bCollideActors=true
	bCollideWorld=true
	bBlockActors=false
	bBlockPlayers=false
}

// --------------------------------------------------------------------------------------------
// spellweb.uc - End of file   
// --------------------------------------------------------------------------------------------