// --------------------------------------------------------------------------------------------
//                 _ _ _____             _ __  __ _           _     _               _           _     _                     
//                | | |  __ \           | |  \/  (_)         | |   | |             (_)         | |   | |                    
//  ___ _ __   ___| | | |  | |_   _  ___| | \  / |_ _ __ ___ | |__ | | _____      ___ _ __ ___ | |__ | | ___     _   _  ___ 
// / __| '_ \ / _ \ | | |  | | | | |/ _ \ | |\/| | | '_ ` _ \| '_ \| |/ _ \ \ /\ / / | '_ ` _ \| '_ \| |/ _ \   | | | |/ __|
// \__ \ |_) |  __/ | | |__| | |_| |  __/ | |  | | | | | | | | |_) | |  __/\ V  V /| | | | | | | |_) | |  __/ _ | |_| | (__ 
// |___/ .__/ \___|_|_|_____/ \__,_|\___|_|_|  |_|_|_| |_| |_|_.__/|_|\___| \_/\_/ |_|_| |_| |_|_.__/|_|\___|(_) \__,_|\___|
//     | |                                                                                                                  
//     |_|                                                                                                                  
// --------------------------------------------------------------------------------------------
// Class Name  : spellDuelMimblewimble
//
// Created on  : 06/25/2002
// 
// Description : 10.6.2.	Mimblewimble - Tongue Twist
//				 This spell fires out a reasonably fast projectile of magic.  When it hits the target 
//				 Wizard, the target will have particles swarming all over them, indicating they are under 
//				 effect of the spell.  Any spell that the target casts for 3 seconds will be flubbed up.  
//				 Instead of saying a spell name, some unrecognizable gobblygook spews from his mouth (funny!), 
//				 and one of 2 possible things will happen:
//
//				 1.	The spell simply sputters to death with a fart sound (70 % chance)
//				 2.	A large explosion occurs (a really LOUD and super disgusting wet fart explosion sound), 
//					severely damaging the target (30% chance)
//
//				 This spell drains a lot of magic strength.
//				
//				 Players can hit a target with multiple Mimblewimble spells, and increase the 3 second 
//				 effectiveness period.
//				
//				 If Harry is hit by Mimblewimble, he has the same particles swarming all around him and 
//				 the effect of the spell is identical as above.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellDuelMimblewimble extends baseSpell;



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
	return HPawn(aHit).HandleSpellDuelMimblewimble( self, vHitLocation );
}

function bool OnSpellHitHarry( Actor aHit, vector vHitLocation )
{
	return Harry(aHit).HandleSpellDuelMimblewimble( self, vHitLocation );
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

	begin:
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell DuelMimblewimble
	SeekSpeed=0.0f
	fxFlyParticleEffectClass=class'duelMimblewimble_fly'
	
	// --- Base Spell
	spellType=SPELL_DuelMimblewimble
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'duelMimblewimble_hit'
//	fxReactParticleEffectClass=class'diffindo_react'
	
	Speed=400.0000

	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellDuelMimblewimble.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


