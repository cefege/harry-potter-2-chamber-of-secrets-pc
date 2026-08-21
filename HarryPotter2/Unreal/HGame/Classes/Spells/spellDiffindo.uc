// --------------------------------------------------------------------------------------------
//                 _ _ _____  _  __  __ _           _                      
//                | | |  __ \(_)/ _|/ _(_)         | |                     
//  ___ _ __   ___| | | |  | |_| |_| |_ _ _ __   __| | ___      _   _  ___ 
// / __| '_ \ / _ \ | | |  | | |  _|  _| | '_ \ / _` |/ _ \    | | | |/ __|
// \__ \ |_) |  __/ | | |__| | | | | | | | | | | (_| | (_) | _ | |_| | (__ 
// |___/ .__/ \___|_|_|_____/|_|_| |_| |_|_| |_|\__,_|\___/ (_) \__,_|\___|
//     | |                                                                 
//     |_|                                                                 
// --------------------------------------------------------------------------------------------
// Class Name  : spellDiffindo
//
// Created on  : 04/02/2002
// 
// Description : The spellDiffindo class implements the Diffindo specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellDiffindo extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function OnSpellShutdown()
{
}

function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellDiffindo( self, vHitLocation );
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
	// --- Spell Diffindo
	Speed=500.0000
	SeekSpeed=6.0f
	fxFlyParticleEffectClass=class'diffindo_fly'
	
	// --- Base Spell
	spellType=SPELL_Diffindo
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'diffindo_hit'
//	fxReactParticleEffectClass=class'diffindo_react'
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellDiffindo.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------