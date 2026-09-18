
// Class Name  : PoisonCloud
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : PoisonCloud AI. The poison cloud that is emitted from a 'live' mushroom
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class PoisonCloud extends HiddenHpawn;

var bool bTouch;
var float fLifetime;
var bool bCanBeThrown;
var float timeSafe;
var bool bCanBeTouched;
var float iDamage;  
var float DamageInterval;
var float waitTime;
var float collideRadius;


// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{
	local HPawn pawn;

	setTimer(fLifetime,false);
}

function timer()
{
	Destroy();
}	

function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	waitTime += DeltaTime;

	if ( waitTime > DamageInterval )
	{
		waitTime = 0;
		bCanBeTouched = true;
	}

	// Not getting a touch if Harry just stands there in the cloud. 
	if ( vsize(location - playerHarry.location) <= (playerHarry.collisionRadius+collisionRadius) )
	{
		if ( bCanBeTouched == true )
		{
			bCanBeTouched = false;

			if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
			{
				playerHarry.TakeDamage (iDamage, Pawn(Owner), location, velocity*1, 'PoisonCloud');
			}
		}

	}

}

function touch (actor other)
{

	local HPawn HPawnHit;	

	// Don't always get a touch see Tick
	if ( other == playerHarry && bCanBeTouched == true )
	{
//		bCanBeTouched = false;

//		if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
//		{
//			if( BOOL_DEBUG_AI ) playerHarry.ClientMessage(" Take Damage  to Harry Instigated By : " $ self);
//			other.TakeDamage (iDamage, Pawn(Owner), location, velocity*1, 'PoisonCloud');
//		}

	}

	else
	{

		if ( other.IsA('HPawn') && 
			 !other.IsA('PoisonCloud') && 
			 !other.IsA('ThrownPoisonCloud') && 
			 !other.IsA('HorklumpsHead') && 
			 !other.IsA('HorklumpsStem') &&
			 bCanBeThrown == true )
		{
			if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
			{
				HPawnHit = HPawn(other);
				if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( self $ " Hit something : " $ HPawnHit );
				HPawnHit.HitByThrownObject (1, HPawnHit, location, velocity*1, 'PoisonCloud');
			}
		}
	}

	PlaySound(Sound'HPSounds.magic_sfx.spell_hit', SLOT_Interact,  1.0, false, 2000.0, 1);

}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

auto state StartHere
{
	begin:

	SetCollisionSize(collideRadius, default.collisionHeight);
}

defaultproperties
{
	 drawType=DT_NONE
     attachedParticleClass(0)=Class'HPParticle.Hork01'
     CollisionRadius=35
     CollisionHeight=32
     bCollideActors=True
     bCollideWorld=True
	 bBlockCamera=false
	 bTouch=True
	 bCanBeTouched=true
	 bCanBeThrown=False
	 fLifetime=1.5
	 iDamage=1
	 DamageInterval=0.5
}
