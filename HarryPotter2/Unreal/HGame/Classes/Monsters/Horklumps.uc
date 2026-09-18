// Class Name  : Horklumps
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : Horklumps AI. The horklumps mushrooms. 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class Horklumps extends Hchar;

// *** Variables

var int randomIdle;
var float waitTime;   // Only one shot of poison at a time
var PoisonCloud aCloud;
var() float triggerCloudDistance;	// distance from Harry
var() float cloudLifetime;			// the lifetime of the cloud (distance from Harry only)
var() float cloudDamage;			// the amount of damage from the cloud
var() float cloudRadius;			// the radius that the cloud does damage
var() float cloudDamageInterval;	// The amount of time between damage while standing in a cloud


// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

function PreBeginPlay()
{

	Super.PreBeginPlay();

	SetCollisionSize(CollisionRadius * DrawScale, CollisionHeight * DrawScale);
}

auto state ReadyAndWaiting
{
	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		waitTime += DeltaTime;

	}


	function ShootPoison ()
	{
		if ( waitTime > 2 )
		{
			waitTime = 0;
			aCloud = spawn(class'PoisonCloud',self,,location+vec(0,0,-10), rotation);
			aCloud.fLifetime = cloudLifetime;
			aCloud.iDamage = cloudDamage;
			aCloud.collideRadius = cloudRadius;
			aCloud.DamageInterval = cloudDamageInterval;
		}
	}

	function ShootPoisonCut ()
	{
		if ( waitTime > 2 )
		{
			waitTime = 0;
			aCloud = spawn(class'PoisonCloudCut',self,,location+vec(0,0,0), rotation);
			aCloud.fLifetime = cloudLifetime;
			aCloud.iDamage = cloudDamage;
			aCloud.collideRadius = cloudRadius;
		}
	}

	function LoseHead()	
	{
		local HorklumpsHead replaceHead;
		local HorklumpsStem replaceStem;
		local vector replaceLocation;
		local rotator replaceRotation;
		local float replaceRadius, replaceHeight, replaceScale;

		replaceLocation = location;
		replaceRotation = rotation;
		replaceRadius = collisionRadius;
		replaceHeight = collisionHeight;
		replaceScale = drawScale;

		destroy();	// I'm destroying right before spawning because it won't let me spawn while it's there

		eVulnerableToSpell = SPELL_None;

		replaceStem = spawn(class'HorklumpsStem',,,location,rotation);
		replaceHead = spawn(class'HorklumpsHead',,,location+vec(0,0,35),rotation);

		if (replaceHead == None || replaceStem == None)
		{
			log("Replace head or stem failed to spawn");
		}
		else
		{
			replaceHead.SetCollisionSize(replaceRadius, replaceHeight);
			replaceHead.drawScale = replaceScale;

			replaceStem.SetCollisionSize(replaceRadius, replaceHeight);
			replaceStem.drawScale = replaceScale;
		}

		PlaySound( sound'HPSounds.Critters_sfx.horklump_mushroom_head_popoff', SLOT_Misc, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );
	}

	function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
	{
		ShootPoisonCut ();
		LoseHead();
		return true;
	}

	function Bump (actor other)
	{

		if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $" Bump ");
		if ( other == playerHarry )
		{
			other.acceleration = vect(0,0,0);
			other.velocity = vect(0,0,0);
			ShootPoison();
//			LoseHead();
		}
	}
begin:
loop:

	Sleep(Rand(2));

	randomIdle = rand(3);

	switch (randomIdle)
	{
	case 0:
		loopAnim ('idle1');
		break;
	case 1:
		loopAnim('idle2');
		break;
	case 2:
		loopAnim('idle3');
		break;
	default:
		break;
	}

	if (baseHud(playerharry.myHud).bCutSceneMode == false)
	{
		if(abs(vsize(location-playerharry.location))<triggerCloudDistance)
		{
			ShootPoison();

			playAnim('Attack');
			FinishAnim();
		}
	}

	goto 'loop';
}


defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skhorklumpsMesh'
	 eVulnerableToSpell=SPELL_Diffindo
     AmbientGlow=65
     CollisionRadius=10
     CollisionHeight=14
     bBlockActors=False
	 DrawScale=1.2
	 bThrownObjectDamage=True
	 triggerCloudDistance=200
	 cloudRadius=35
	 cloudLifetime=1.5
	 cloudDamage=1
	 cloudDamageInterval=2

}
