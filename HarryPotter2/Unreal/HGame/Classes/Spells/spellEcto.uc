// --------------------------------------------------------------------------------------------
//                 _ _ ______      _                        
//                | | |  ____|    | |                       
//  ___ _ __   ___| | | |__    ___| |_  ___      _   _  ___ 
// / __| '_ \ / _ \ | |  __|  / __| __|/ _ \    | | | |/ __|
// \__ \ |_) |  __/ | | |____| (__| |_| (_) | _ | |_| | (__ 
// |___/ .__/ \___|_|_|______|\___|\__|\___/ (_) \__,_|\___|
//     | |                                                  
//     |_|                                                  
// --------------------------------------------------------------------------------------------
// Class Name  : spellEcto
//
// Created on  : 04/15/2002
// 
// Description : The spellEcto class implements the Ecto spell specific code
//				 Harry can not cast Ecto, instead ghosts cast Ecto to re-grow ectoplasma.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellEcto extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


var (VisualFX)ParticleFX		fxHitHarryParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHitHarryParticleEffectClass;

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function OnSpellInit()
{
	// If the spell's target is harry then make the SeekSpeed much smaller
	if( TargetActor.IsA('Harry') )
		SeekSpeed = 0.5f;
}

function OnSpellShutdown()
{

}


function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellEcto( self, vHitLocation );
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
	
	function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
	{
		aHit.TakeDamage( 15, instigator, vect(0,0,0), vect(0,0,0), '' );
		fxHitHarryParticleEffect = spawn( fxHitHarryParticleEffectClass );
		return true; // we have a valid hit
	}

	begin:
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell Ecto
	Speed=400.0f
	SeekSpeed=2.5f

	fxFlyParticleEffectClass=class'Ecto_fly'
	fxHitHarryParticleEffectClass=class'Skurge_hitHarry'
	
	// --- Base Spell
	spellType=SPELL_Ecto
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'Ecto_hit'
	fxReactParticleEffectClass=class'Ecto_react'

	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellEcto.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------