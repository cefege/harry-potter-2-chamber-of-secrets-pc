// --------------------------------------------------------------------------------------------
//                 _ _  _____ _                                         
//                | | |/ ____| |                                        
//  ___ _ __   ___| | | (___ | | ___   _ _ __  __ _  ___     _   _  ___ 
// / __| '_ \ / _ \ | |\___ \| |/ / | | | '__|/ _` |/ _ \   | | | |/ __|
// \__ \ |_) |  __/ | |____) |   <| |_| | |  | (_| |  __/ _ | |_| | (__ 
// |___/ .__/ \___|_|_|_____/|_|\_\\__,_|_|   \__, |\___|(_) \__,_|\___|
//     | |                                     __/ |                    
//     |_|                                    |___/                     
// --------------------------------------------------------------------------------------------
// Class Name  : spellSkurge
//
// Created on  : 04/02/2002
// 
// Description : The spellSkurge class implements the Skurge specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellSkurge extends baseSpell;

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
	return HPawn(aHit).HandleSpellSkurge( self, vHitLocation );
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
	// --- Spell Skurge
	Speed=400.0f
	SeekSpeed=4.0f

	fxFlyParticleEffectClass=class'skurge_fly'
	
	// --- Base Spell
	spellType=SPELL_Skurge
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'SkurgeSpell_hit'
	fxReactParticleEffectClass=none
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellSkurge.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------