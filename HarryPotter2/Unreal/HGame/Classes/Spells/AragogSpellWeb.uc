
// Class Name  : AragogSpellWeb
//
// Created on  : 06/07/2002
// Authored by : Janet Weddle
// 
// Description : The AragogSpellWeb class implements the Aragog Web spell specific code
//				 Harry can not cast Web, instead Aragog casts at his web to fix it
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogSpellWeb extends baseSpell;

// --------------------------------------------------------------------------------------------
// *** Variables

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function OnSpellShutdown()
{
}

function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
{
	return true;
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
		UpdateRotationWithSeeking( fTimeDelta );		

		// Update our fly particles
		if( fxFlyParticleEffect != None )
			fxFlyParticleEffect.SetLocation( location );
	}

	begin:
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell Skurge
	SeekSpeed=3.0f
	
	fxFlyParticleEffectClass=class'Ecto_fly'
	
	// --- Base Spell
	spellType=SPELL_Ecto
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'Ecto_hit'
	fxReactParticleEffectClass=class'Ecto_react'

	// --- ParticleFX
	Speed=800.0f
    DrawType=DT_None
}
//----------------------------------------------------------------------