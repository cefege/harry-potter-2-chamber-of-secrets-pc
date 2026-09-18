// --------------------------------------------------------------------------------------------
//                 _ _ _                                               
//                | | | |                                              
//  ___ _ __   ___| | | |     _   _ _ __ ___   ___  ___     _   _  ___ 
// / __| '_ \ / _ \ | | |    | | | | '_ ` _ \ / _ \/ __|   | | | |/ __|
// \__ \ |_) |  __/ | | |____| |_| | | | | | | (_) \__ \ _ | |_| | (__ 
// |___/ .__/ \___|_|_|______|\__,_|_| |_| |_|\___/|___/(_) \__,_|\___|
//     | |                                                             
//     |_|                                                             
// --------------------------------------------------------------------------------------------
// Class Name  : spellLumos
//
// Created on  : 05/13/2002
// 
// Description : The spellLumos class implements the Lumos specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellLumos extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{	
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellLumos( self, vHitLocation );
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

		playerHarry.clientMessage("Lumos: BeginState() StateFlyingToTarget");
	}
	
	
	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
		
		UpdateRotationWithSeeking( fTimeDelta );
		
		// Update our fly particles
		if( fxFlyParticleEffect != None )
			fxFlyParticleEffect.SetLocation( location );
	}
}


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell Lumos
	Speed=400.0000
	SeekSpeed=5.0f
	fxFlyParticleEffectClass=class'lumos_fly'
	
	// --- Base Spell
	spellType=SPELL_Lumos
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'lumos_hit'
//	fxReactParticleEffectClass=class'lumos_react'
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellLumos.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------