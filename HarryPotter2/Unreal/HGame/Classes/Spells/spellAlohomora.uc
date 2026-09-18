// --------------------------------------------------------------------------------------------
//                 _ _          _       _                                                     
//                | | |   /\   | |     | |                                                    
//  ___ _ __   ___| | |  /  \  | | ___ | |__   ___  _ __ ___   ___  _ __  __ _     _   _  ___ 
// / __| '_ \ / _ \ | | / /\ \ | |/ _ \| '_ \ / _ \| '_ ` _ \ / _ \| '__|/ _` |   | | | |/ __|
// \__ \ |_) |  __/ | |/ ____ \| | (_) | | | | (_) | | | | | | (_) | |  | (_| | _ | |_| | (__ 
// |___/ .__/ \___|_|_/_/    \_\_|\___/|_| |_|\___/|_| |_| |_|\___/|_|   \__,_|(_) \__,_|\___|
//     | |                                                                                    
//     |_|                                                                                    
// --------------------------------------------------------------------------------------------
// Class Name  : spellAlohomora
//
// Created on  : 04/02/2002
// 
// Description : The spellAlohomora class implements the Alohomora specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellAlohomora extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
		
	// Init our seeking current Dir var
	CurrentDir = normal(vector(Rotation) + vec(0,0,0.3f) );
}

function OnSpellShutdown()
{
}


function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellAlohomora( self, vHitLocation );
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
	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
		
		// update our seeking direction
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
	// --- Spell Alohomora
	Speed=500.0000
	SeekSpeed=5.0f
	fxFlyParticleEffectClass=class'aloh_fly'
	
	// --- Base Spell
	spellType=SPELL_Alohomora
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	ImpactSound=Sound'HPSounds.magic_sfx.ALO_hit'

	fxHitParticleEffectClass=class'aloh_hit'
//	fxReactParticleEffectClass=class'aloh_react'
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellAlohomora.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------