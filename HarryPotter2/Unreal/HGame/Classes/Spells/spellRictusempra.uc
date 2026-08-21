// --------------------------------------------------------------------------------------------
//                 _ _ _____  _      _                                                            
//                | | |  __ \(_)    | |                                                           
//  ___ _ __   ___| | | |__) |_  ___| |_ _   _ ___  ___ _ __ ___  _ __  _ __  __ _     _   _  ___ 
// / __| '_ \ / _ \ | |  _  /| |/ __| __| | | / __|/ _ \ '_ ` _ \| '_ \| '__|/ _` |   | | | |/ __|
// \__ \ |_) |  __/ | | | \ \| | (__| |_| |_| \__ \  __/ | | | | | |_) | |  | (_| | _ | |_| | (__ 
// |___/ .__/ \___|_|_|_|  \_\_|\___|\__|\__,_|___/\___|_| |_| |_| .__/|_|   \__,_|(_) \__,_|\___|
//     | |                                                       | |                              
//     |_|                                                       |_|                              
// --------------------------------------------------------------------------------------------
// Class Name  : spellRictusempra
//
// Created on  : 04/02/2002
// 
// Description : The spellRictusempra class implements the Rictusempra specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellRictusempra extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions

function OnSpellInit()
{
	local float fDistMod;
	
	// Set our Direction vector
	CurrentDir = vector( Rotation );
	
	// compute a distance modifier depending upon how close we are to our target 
	// ( the closer we are the straighter it will aim to our target )
	fDistMod = vsize( TargetActor.location - location  ) / 800;
	
	if(fDistMod > 1.0)
		fDistMod = 1.0f;
	
	// Alter our seek speed depending upon how close we are to our target 
	// (so the closer we are the faster we will seek our target)
	SeekSpeed += 1.0 - fDistMod;

	// Have the direction vector altered in a cone like shape randomly
	CurrentDir.x += (FRand() - 0.4f) * fDistMod;
	CurrentDir.y += (FRand() - 0.4f) * fDistMod;
	CurrentDir.z += (FRand() * 0.5f) * fDistMod;
	
	playerHarry.clientmessage( " fDistMod = " $fDistMod $" curDir = " $CurrentDir );
	SetRotation( rotator( CurrentDir ) );
}


function OnSpellShutdown()
{
}


function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellRictusempra( self, vHitLocation );
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
		Velocity	 = vector(Rotation) * Speed;
		Acceleration = vector(Rotation) * 10;
	}
	
	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
		
		UpdateRotationWithSeeking( fTimeDelta );

//		if( fxFlyingGesture != None )
//			fxFlyingGesture.SetLocation( Location );
		
		// Update our fly particles
		if( fxFlyParticleEffect != None )
			fxFlyParticleEffect.SetLocation( location );

	/*	
		switch( iStage )
		{
			case 0:
				// smoothly goto our destination
				vPositionCurrent += (vPositionTarget - vPositionCurrent ) * 4.0f * fTimeDelta;
				
				if( vsize(vPositionCurrent - vPositionTarget) < 1.0f )
				{
					vPositionTarget	 = SpellTarget.Location;
					iStage++;
				}
				break;
			
			case 1:
				// smoothly goto our destination
				vPositionCurrent += (vPositionTarget - vPositionCurrent ) * 8.0f * fTimeDelta;
				break;
		}
		
		fxFlyingGesture.SetLocation( vPositionCurrent );
		SetLocation( vPositionCurrent );
	*/
	}
	
	begin:
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell Rictusempra
	Speed=500.0f
	SeekSpeed=5.0f
	fxFlyParticleEffectClass=class'rictusempra_fly'

//	CastSound=Sound'HPSounds.AllDialog.spell_cast'
//	CastSound=Sound(DynamicLoadObject("AllDialog.spell_cast", class'Sound'));

	// --- Base Spell
	spellType=SPELL_Rictusempra
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells1"
	QuietSpellIncantation="spells10"
	

	fxHitParticleEffectClass=class'rictusempra_hit'

//	fxReactParticleEffectClass=class'rictusempra_react'
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellRictusempra.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


