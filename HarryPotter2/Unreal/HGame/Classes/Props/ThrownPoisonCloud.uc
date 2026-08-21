
// Class Name  : ThrownPoisonCloud
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : ThrownPoisonCloud AI. The poison cloud that emits from a thrown horklump mushroom
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class ThrownPoisonCloud extends HiddenHpawn;

var bool bTouch;
var float fLifetime;
var bool bCanBeThrown;
var float timeSafe;
var bool bCanBeTouched;


// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

function preBeginPlay()
{
	Super.preBeginPlay();

	SetCollision(false,false,false);
}

function postBeginPlay()
{
	local HPawn pawn;
	local vector vTargetDir;

	setTimer(fLifetime,false);

	foreach AllActors( class'HPawn', pawn )
	{
		if ( pawn == playerHarry && bCanBeTouched == true )
		{
			bCanBeTouched = false;

			if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
			{
				if( BOOL_DEBUG_AI ) playerHarry.ClientMessage(" Take Damage  to Harry Instigated By : " $ self);
				pawn.TakeDamage (1, Pawn(Owner), location, velocity*1, 'PoisonCloud');
	
			}
		}


//		if ( !pawn.IsA('PoisonCloud') && 
//		if ( !pawn.IsA('HorklumpsHead') && 
//			 !pawn.IsA('HorklumpsStem') &&
		if (	 bCanBeThrown == true )
		{
			if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
			{
				vTargetDir = location - pawn.location;
	
				if( vsize(vTargetDir) < CollisionRadius*4+pawn.CollisionRadius )
				{
					if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( self $ " Hit " $ pawn );
					pawn.HitByThrownObject (1, pawn, location, velocity*1, 'PoisonCloud');
				}
			}
		}

	}
	PlaySound(Sound'HPSounds.magic_sfx.spell_hit', SLOT_Interact,  1.0, false, 2000.0, 1);

}

function timer()
{
	Destroy();
}	
/*
function touch (actor other)
{

	Super.touch(other);
	// Touch won't work for the thrown cloud. Apparently you can't spawn something and try to get collision
	// on something that is already inside. Go figure...

}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}
*/
defaultproperties
{
	 drawType=DT_NONE
     attachedParticleClass(0)=Class'HPParticle.Hork03'
	 attachedParticleClass(0)=Class'HPParticle.Hork04'
//     CollisionRadius=55
	 CollisionRadius=25
     CollisionHeight=32
	 bBlockActors=False
	 bBlockPlayers=False
	 bBlockCamera=false
     bCollideActors=False
     bCollideWorld=True
	 bTouch=True
	 bCanBeTouched=true
	 bCanBeThrown=true
	 fLifetime=1.5
}



