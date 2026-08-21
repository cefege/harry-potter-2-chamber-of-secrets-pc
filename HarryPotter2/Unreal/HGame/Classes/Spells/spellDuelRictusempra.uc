// --------------------------------------------------------------------------------------------
//                 _ _ _____             _ _____  _      _                                                            
//                | | |  __ \           | |  __ \(_)    | |                                                           
//  ___ _ __   ___| | | |  | |_   _  ___| | |__) |_  ___| |_ _   _ ___  ___ _ __ ___  _ __  _ __  __ _     _   _  ___ 
// / __| '_ \ / _ \ | | |  | | | | |/ _ \ |  _  /| |/ __| __| | | / __|/ _ \ '_ ` _ \| '_ \| '__|/ _` |   | | | |/ __|
// \__ \ |_) |  __/ | | |__| | |_| |  __/ | | \ \| | (__| |_| |_| \__ \  __/ | | | | | |_) | |  | (_| | _ | |_| | (__ 
// |___/ .__/ \___|_|_|_____/ \__,_|\___|_|_|  \_\_|\___|\__|\__,_|___/\___|_| |_| |_| .__/|_|   \__,_|(_) \__,_|\___|
//     | |                                                                           | |                              
//     |_|                                                                           |_|                              
// --------------------------------------------------------------------------------------------
// Class Name  : spellDuelRictusempra
//
// Created on  : 06/25/2002
// 
// Description : 10.6.1.	Rictusempra - Gut Punch
//				 This spell is identical to the normal Rictusempra, except that casting it drains 
//				 a little bit of magic strength.
//
//				 This is the basic combat spell.  It is mostly used as a defensive against monsters 
//				 and against your opponent in a Wizard Duel.   It shoots an extremely fast projectile 
//				 delivers a punch in the gut, winding the target slightly and causing damage.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellDuelRictusempra extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
}

function OnSpellShutdown()
{
}

function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	return HPawn(aHit).HandleSpellDuelRictusempra( self, vHitLocation );
}

function bool OnSpellHitHarry( Actor aHit, vector vHitLocation )
{
	return Harry(aHit).HandleSpellDuelRictusempra( self, vHitLocation );
}


// --------------------------------------------------------------------------------------------
// *** States

state auto StateIdle
{
	begin:
	GotoState('StateFlying');
}

state StateFlying
{
	function BeginState()
	{
		// set our velocity in the direction we are facing * speed
		Velocity	 = vector(Rotation) * Speed;
		Acceleration = Velocity;
	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
		
		UpdateRotationWithSeeking( fTimeDelta );

		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );
		}
	}
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell DuelRictusempra
	SeekSpeed=0.0f
	fxFlyParticleEffectClass=class'duelRictusempra_fly'
	
	// --- Base Spell
	spellType=SPELL_DuelRictusempra
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'duelRictusempra_hit'
	
	Speed=500.0000

	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellDuelRictusempra.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


