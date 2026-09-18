// --------------------------------------------------------------------------------------------
//                 _ _  _____                         _  __                      
//                | | |/ ____|                       (_)/ _|                     
//  ___ _ __   ___| | | (___  _ __   ___  _ __   __ _ _| |_ _   _     _   _  ___ 
// / __| '_ \ / _ \ | |\___ \| '_ \ / _ \| '_ \ / _` | |  _| | | |   | | | |/ __|
// \__ \ |_) |  __/ | |____) | |_) | (_) | | | | (_| | | | | |_| | _ | |_| | (__ 
// |___/ .__/ \___|_|_|_____/| .__/ \___/|_| |_|\__, |_|_|  \__, |(_) \__,_|\___|
//     | |                   | |                 __/ |       __/ |               
//     |_|                   |_|                |___/       |___/                
// --------------------------------------------------------------------------------------------
// Class Name  : spellSpongify
//
// Created on  : 04/02/2002
// 
// Description : The spellSpongify class implements the Spongify specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellSpongify extends baseSpell;


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
	CurrentDir = normal(vector(Rotation) + vec(0,0,0.25f) );
}

function OnSpellShutdown()
{
}



function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellSpongify( self, vHitLocation );
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
	// --- Spell Spongify
	Speed=800.0000
	SeekSpeed=8.0
	fxFlyParticleEffectClass=class'spongify_fly'
	
	// --- Base Spell
	spellType=SPELL_Spongify
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'spongify_hit'
//	fxReactParticleEffectClass=class'spongify_react'
	
	// --- Projectile
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellSpongify.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------
