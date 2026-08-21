// --------------------------------------------------------------------------------------------
//                 _ _ _____             _ ______                 _ _ _                                               
//                | | |  __ \           | |  ____|               | | (_)                                              
//  ___ _ __   ___| | | |  | |_   _  ___| | |__  __  ___ __   ___| | |_  __ _ _ __ _ __ ___  _   _ ___     _   _  ___ 
// / __| '_ \ / _ \ | | |  | | | | |/ _ \ |  __| \ \/ / '_ \ / _ \ | | |/ _` | '__| '_ ` _ \| | | / __|   | | | |/ __|
// \__ \ |_) |  __/ | | |__| | |_| |  __/ | |____ >  <| |_) |  __/ | | | (_| | |  | | | | | | |_| \__ \ _ | |_| | (__ 
// |___/ .__/ \___|_|_|_____/ \__,_|\___|_|______/_/\_\ .__/ \___|_|_|_|\__,_|_|  |_| |_| |_|\__,_|___/(_) \__,_|\___|
//     | |                                            | |                                                             
//     |_|                                            |_|                                                             
// --------------------------------------------------------------------------------------------
// Class Name  : spellDuelExpelliarmus
//
// Created on  : 07/07/2002
// 
// Description : 10.6.3.	Expelliarmus - Spell Rebound 
//				 Casting this spell will allow the caster to reverse any incoming spell by sending 
//				 it back to its originator.  
//
//				 It must be cast at the moment that the incoming projectile is about to hit Harry.  
//				 When cast, Harry will cry out "Expelliarmus!" while striking his wand at the incoming 
//				 projectile, appearing to strike it like one would a cricket bat in one hand.  
//
//				 If it successfully hits, a bright flash will appear at the strike point, sending the 
//				 spell projectile in the direction that Harry is facing.  
//
//				 If Harry attempts to cast the spell when there is no incoming projectile, he will not 
//				 say the name of the spell, but will swing his wand, hitting nothing but air.
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellDuelExpelliarmus extends baseSpell;



// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions

function color Col(float R, float G, float B)
{
	local color C;
	C.R = R; C.G = G; C.B = B;
	return C;
}

function PostBeginPlay()
{
	Super.PostBeginPlay();

//	if( owner.IsA('Harry') )
//		Harry(owner).bReboundingSpells = true;
//	else if( owner.IsA('Duellist') )
//		Duellist(owner).bReboundingSpells = true;
}

function OnSpellInit()
{
}

function OnSpellShutdown()
{
}

function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	return HPawn(aHit).HandleSpellDuelExpelliarmus( self, vHitLocation );
}

function bool OnSpellHitHarry( Actor aHit, vector vHitLocation )
{
	return Harry(aHit).HandleSpellDuelExpelliarmus( self, vHitLocation );
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
	}

	event Tick( float fTimeDelta )
	{
		local rotator rot;

		super.Tick( fTimeDelta );
		
		// place this spell in front of instigator
		SetLocation( SpellWand.GetWandEndPoint() );
		
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
	// --- Spell DuelExpelliarmus
	SeekSpeed=0.0f
	SpellLifeTime=1.0f
	fxFlyParticleEffectClass=None

	// --- Base Spell
	spellType=SPELL_DuelExpelliarmus
	spellIcon=Texture'alohoSpellIcon'
	Speed=0.0f
	
	SpellIncantation="spells3"
	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'lumos_hit'
//	fxReactParticleEffectClass=class'diffindo_react'
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellDuelExpelliarmus.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


