// --------------------------------------------------------------------------------------------
//  ______      _               _                                          
// |  ____|    | |             | |                                         
// | |__    ___| |_  ___  _ __ | | __ _ ___ _ __ ___   __ _     _   _  ___ 
// |  __|  / __| __|/ _ \| '_ \| |/ _` / __| '_ ` _ \ / _` |   | | | |/ __|
// | |____| (__| |_| (_) | |_) | | (_| \__ \ | | | | | (_| | _ | |_| | (__ 
// |______|\___|\__|\___/| .__/|_|\__,_|___/_| |_| |_|\__,_|(_) \__,_|\___|
//                       | |                                               
//                       |_|                                               
// --------------------------------------------------------------------------------------------
// Class Name  : Ectoplasma
//
// Created on  : 04/08/2002
// 
// Description : The Ectoplasma is a blob like substance that slows Harry's movement down and hurts him.
//				 You can cast Skurge on the Ectoplasma and it will shrink then become hidden.
//				 Enemies may revive the Ectoplasma causing it to  grow and "show" itself.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class Ectoplasma extends Characters;


// --------------------------------------------------------------------------------------------
// *** Variables


// Settings
var() float				fShrinkTime;			// how long will it take to shrink down to 0.0
var() float				fGrowTime;				// how long will it take to grow

var() float 			fDamageTimer;			// how often to we cause damage to harry
var() int				iDamage;				// how much damage do we cause to harry
var() bool				bGrowOnEvent;			// Grow on event? else Shrink on event

var float				fTimeSpent;
var actor				aSlimedHPawn;			// save the HPawn we are sliming

var() sound				ShrinkSound;			// sound ectoplasma makes when shrinking
var() sound				BumpSound;				// sound ectoplasma makes when something bumps into it

var float				fxParticlesPerSecond;	// how many particles per second do we want to produce
var ParticleFX			fxHit;					// ParticleFX hit ref
var ParticleFX			fxReact;				// ParticleFX react ref
var class<ParticleFX>	fxHitClass;				// class to create hit FX with
var class<ParticleFX>	fxReactClass;			// class to create react FX with

// --------------------------------------------------------------------------------------------
// *** Constants

const PARTICLES_PER_SECOND_BASE = 60;

// --------------------------------------------------------------------------------------------
// *** Functions

function PreBeginPlay()
{
	Super.PreBeginPlay();

	// Set our collision to collide actors = true, block actors or players == false
	SetCollision( true, false, false );

	// Find harry and save him
	playerHarry = Harry(Level.playerHarryActor);
	

	// Setup a timer for how oftin we cause damage to harry (if we are touching him)
	SetTimer(fDamageTimer, true);
	
	if( InitialState == 'stateHiding' )
	{
		// don't wait for the hiding state to shrink the ecto and hide
		DrawScale = 0.0f;
		bHidden   = true;
	}
}

function Bump( actor other )
{
	PlaySound( BumpSound, SLOT_Misc );
}

function float GetDefaultDrawScale()
{
	// this function is to get around a bug where if you use "default.DrawScale" 
	// it will use the parent's if it is used in the parents function, 
	// not the derived class's version of default.DrawScale
	return default.DrawScale;
}

event Destroyed()
{	
	// make sure our fx is shutdown
	if( fxHit != None )
		fxHit.Shutdown();
	
	if( fxReact != None )
		fxReact.Shutdown();

	Super.Destroyed();
}

function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	return false; // invalid hit
}

function bool HandleSpellSkurge( optional baseSpell spell, optional vector vHitLocation )
{
	if( IsInState('stateShowing') )
	{
		// Send out an event that we are going to hide
		TriggerEvent( Event, none, none );
		
		// create the skurge reaction
		fxHit = spawn( fxHitClass );	
		fxHit.SetLocation( location );
		fxHit.SetOwner( Self );
		
		
		fxReact = spawn( fxReactClass );
		fxReact.SetLocation( location );
		fxReact.SetOwner( Self );
		
		//DEBUG
//		playerHarry.ClientMessage( "*********************** Returning true!!! because this ecto is showing!");
		
		// Have the skurge spell NOT make a hit effect
		spell.fxHitParticleEffectClass = None;

		// goto the hiding state
		GotoState('stateHiding');
		return true;
	}

	// DEBUG
//	playerHarry.ClientMessage( "*********************** Returning false!!!! because this ecto is hidden" );
	
	// we do not want skurge to be relevant if we are not in stateShowing
	return false;
}

function bool HandleSpellEcto( optional baseSpell spell, optional vector vHitLocation )
{
	if( IsInState('stateHiding') )
	{
		// A ghost or something has hit ectoplasma with ecto!!! so lets grow back!
		GotoState('stateShowing');
		return true;	
	}	
	// we do not want Ecto to be relevant if we are not in stateShowing
	return false;
}

function Trigger( actor Other, pawn EventInstigator )
{
	//If we receive an event grow or shrink depending upon the ectoplasma settings
	if( bGrowOnEvent )
	{
		GotoState('stateShowing');
	}
	else
	{
		GotoState('stateHiding');
	}
}

function UpdateFX()
{
	local vector colRotated;
	
	// rotate our collision h,w,d so that it corisponds with rotated ectoplasma
	colRotated = vec(CollisionRadius, CollisionRadius, CollisionHeight ) >> rotation;
	
	// affect our FX depending upon our DrawScale
	fxHit.SourceHeight.Base			= colRotated.x * 2.0f  * DrawScale;
	fxHit.SourceWidth.Base			= colRotated.y * 2.0f  * DrawScale;
	fxHit.SourceDepth.Base			= colRotated.z * 2.0f  * DrawScale;
	fxHit.ParticlesPerSec.Base		= fxParticlesPerSecond * DrawScale;
	
	fxReact.SourceHeight.Base		= colRotated.x * 2.0f  * DrawScale;
	fxReact.SourceWidth.Base		= colRotated.y * 2.0f  * DrawScale;
	fxReact.SourceDepth.Base		= colRotated.z * 2.0f  * DrawScale;
	fxReact.ParticlesPerSec.Base	= fxParticlesPerSecond * DrawScale;
}

/*
// create more ectoplasma explosions randomply within the bounding box
function CreateRandomSpurt()
{
	local ParticleFX	fxSpurt;
	local BoundingBox	bbArea;
	local vector		vEmit;
	
	// get the area of our bounding box and choose an emit location
	bbArea  = GetWorldCollisionBox(true);
	vEmit.x = bbArea.Min.x + ((bbArea.Max.x - bbArea.Min.x) * FRand());
	vEmit.y = bbArea.Min.y + ((bbArea.Max.y - bbArea.Min.y) * FRand());
	vEmit.z = bbArea.Min.z + ((bbArea.Max.z - bbArea.Min.z) * 0.5f );
	
	// Spawn HitParticle Effects
	fxSpurt = spawn( class'Skurge_hit' );
	fxSpurt.SetLocation( vEmit );
}
*/

// --------------------------------------------------------------------------------------------
// *** States


state auto stateIdle
{
	begin:
	GotoState('stateShowing');
}


state() stateShowing
{
	function BeginState()
	{
		bCollideWorld		= true;			// we need to have the ectoplasma stop gridMovers
		eVulnerableToSpell	= SPELL_Skurge;
		fTimeSpent			= 0.0f;

		// start the ambient sound
		if( AmbientSound != none )
		{
			// We need to loop this ambientSound
			AmbientSound = default.AmbientSound;
			//playerHarry.ClientMessage("Started the Ecto Ambient Sound:" $AmbientSound $" fTimeSpent= " $fTimeSpent );
		}
	}
	
	function Tick( float fTimeDelta )
	{
		// If we need to grow then do so
		if( DrawScale < GetDefaultDrawScale() )
		{
			// update how much time has been spent
			fTimeSpent += fTimeDelta;
			
			// set our drawScale according to our shrinkTime
			DrawScale = (fTimeSpent / fGrowTime);
			
			if( DrawScale >= GetDefaultDrawScale() )
			{
				// set our DrawScale to normal size
				DrawScale = GetDefaultDrawScale();
			}

			// update our particle FX
			UpdateFX();

			
			//DEBUG
			
			//playerHarry.ClientMessage("fxHit Age is -> " $fxHit.Age );
			//playerHarry.ClientMessage("Growing -> DrawScale = " $DrawScale $" fTimeSpent= " $fTimeSpent );

		}
	}
	
	simulated function Timer()
	{
		// If we are touching harry then hurt him
		if( aSlimedHPawn != none && aSlimedHPawn.IsA('Harry') )
		{
			Harry(aSlimedHPawn).TakeDamage(iDamage, Self, location, vec(0,0,0) , 'ectoplasma' );
			
			//DEBUG
			playerHarry.ClientMessage("DamageTimer " $fDamageTimer $", taking damage " $iDamage $", current health is " $playerHarry.GetHealthStatusItem().nCount );
		}
	}

	function Touch( actor other )
	{
		//DEBUG
//		playerHarry.ClientMessage("Touch on the ectoplasma!");
		
		// If harry touches ectoplasma then we need to slow him down.
		if( other.IsA('Harry') )
		{
			Harry(other).EctoRefAdd();
			aSlimedHPawn = other;
		}
	}
	
	function UnTouch( actor other )
	{
		//DEBUG
//		playerHarry.ClientMessage("UnTouch on the ectoplasma!");

		// If harry touches ectoplasma then we need to slow him down.
		if( other.IsA('Harry') )
		{
			Harry(other).EctoRefSub();
			aSlimedHPawn = none;
		}
	}
	
	begin:
	LoopAnim( animsequence );
	AnimFrame = RandRange( 0, 0.95 );
}


state() stateHiding
{
	function BeginState()
	{
		bCollideWorld		= false;
		eVulnerableToSpell	= SPELL_None;
		fTimeSpent			= 0.0f;
	
		// Just in case we were still sliming harry (he may have casted skurge while inside ectoplasma)
		if( aSlimedHPawn != None && aSlimedHPawn.IsA('Harry') )
		{
			Harry(aSlimedHPawn).EctoRefSub();
		}
		aSlimedHPawn = none;
		
		PlaySound( ShrinkSound, SLOT_Misc, 0.75f );
	}
	
	function EndState()
	{
		bHidden   = false;
	}

	function Tick( float fTimeDelta )
	{
		// shrink then hide
		if( bHidden == false )
		{
			// update how much time has been spent
			fTimeSpent += fTimeDelta;
			
			// set our drawScale according to our shrinkTime
			DrawScale = GetDefaultDrawScale() - (fTimeSpent / fShrinkTime );

			if( DrawScale <= 0.0f )
			{
				DrawScale = 0.0f;
				bHidden   = true;
				
				// stop our particle emitting
				fxHit.Shutdown();
				fxReact.Shutdown();
			
				playerHarry.ClientMessage("Time spent shrinking ectoplasma = " $fTimeSpent );

				// stop the sound if it is playing
				if( AmbientSound != none )
				{
					AmbientSound = None;// we need to stop a looping ambient sound
				}
			}
			
			// update our particle FX
			UpdateFX();

			//DEBUG
//			playerHarry.ClientMessage("fxHit Age is -> " $fxHit.Age );
//			playerHarry.ClientMessage("Shrinking -> DrawScale = " $DrawScale $" fTimeSpent= " $fTimeSpent );
		}
			
	}

	function Touch( actor other )
	{
		// if its a ghost then show again?
	}
	
	begin:
}


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
     fShrinkTime=0.85
     fGrowTime=2.5
     fDamageTimer=0.5
     iDamage=10
     ShrinkSound=Sound'HPSounds.Ch2Skurge.ecto_hit'
     BumpSound=Sound'HPSounds.Ch2Skurge.ecto_blocking_movement'
     fxParticlesPerSecond=40
     fxHitClass=Class'HPParticle.Skurge_hit'
     fxReactClass=Class'HPParticle.Skurge_react'
     Physics=PHYS_None
     AnimSequence=idle1
     AmbientSound=Sound'HPSounds.Ch2Skurge.ecto_idle2'
     eVulnerableToSpell=SPELL_Skurge
     Style=STY_Translucent
     Texture=IceTexture'HPParticle.hp_fx.General.EctoplasmFX'
     Mesh=SkeletalMesh'HPModels.skectoplasmaMesh'
     AmbientGlow=255
     bUnlit=True
     bRandomFrame=True
     bMeshEnviroMap=True
     CollisionRadius=42
     CollisionHeight=10
     bBlockActors=False
     bBlockPlayers=False
	 bDoEyeBlinks=false

	 bGestureFaceHorizOnly=false
}

// --------------------------------------------------------------------------------------------
// Ectoplasma.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------
