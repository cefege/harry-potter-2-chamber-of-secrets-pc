// --------------------------------------------------------------------------------------------
//                 _ _ ______ _ _                      _                      
//                | | |  ____| (_)                    | |                     
//  ___ _ __   ___| | | |__  | |_ _ __   ___ _ __   __| | ___      _   _  ___ 
// / __| '_ \ / _ \ | |  __| | | | '_ \ / _ \ '_ \ / _` |/ _ \    | | | |/ __|
// \__ \ |_) |  __/ | | |    | | | |_) |  __/ | | | (_| | (_) | _ | |_| | (__ 
// |___/ .__/ \___|_|_|_|    |_|_| .__/ \___|_| |_|\__,_|\___/ (_) \__,_|\___|
//     | |                       | |                                          
//     |_|                       |_|                                          
// --------------------------------------------------------------------------------------------
// Class Name  : spellFlipendo
//
// Created on  : 04/01/2002
// 
// Description : The spellFlipendo class implements the Flipendo specific code
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellFlipendo extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


event BeginEvent()	{}
event EndEvent()	{}
event KilledBy( pawn EventInstigator )	{}

function OnSpellShutdown()
{
}


function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	return HPawn(aHit).HandleSpellFlipendo( self, vHitLocation );
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
		Velocity = vector(Rotation) * Speed;
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
		
/*		switch( iStage )
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

// Grid mover could be pushed by Flipendo
function bool IsRelevantToMover()
{
	return true;
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell Flipendo
	Speed=500.0f
	SeekSpeed=8.0f
	fxFlyParticleEffectClass=class'flip_fly'

	// --- Base Spell
	spellType=SPELL_Flipendo
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells1"
	QuietSpellIncantation="spells10"
	
	fxHitParticleEffectClass=class'flip_hit'
//	fxReactParticleEffectClass=class'flip_react'
	
	// --- ParticleFX
    DrawType=DT_None
}

// --------------------------------------------------------------------------------------------
// spellFlipendo.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


