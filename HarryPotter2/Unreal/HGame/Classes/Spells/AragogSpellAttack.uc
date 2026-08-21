
// Class Name  : AragogSpellAttack
//
// Created on  : 07/08/2002
// Authored by : Janet Weddle
// 
// Description : The AragogSpellAttack class implements the Aragog's Attack spell specific code
//				 Harry can not cast AragogSpellAttack, instead the Aragog casts at Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogSpellAttack extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables

var (VisualFX)ParticleFX		fxHeadParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHeadParticleEffectClass;

var() float fIncreaseHitTimeDistance;
var() float	fHitTimeIncrement;
var int iDamage;
var vector hitTarget;		// The location where the spell should hit

// --------------------------------------------------------------------------------------------
// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	
	// Create our Head FX
//	fxHeadParticleEffect = spawn( fxHeadParticleEffectClass );
//	fxHeadParticleEffect.SetLocation( Location );
//	fxHeadParticleEffect.SetRotation( fxHeadParticleEffect.default.rotation );

	//SetCollision( true, false, false );

 	//Create our Fly FX
	fxFlyParticleEffect = spawn( fxFlyParticleEffectClass );
	fxFlyParticleEffect.SetLocation( Location );
	fxFlyParticleEffect.SetRotation( fxFlyParticleEffect.default.rotation );

	PlaySound( sound'HPSounds.critters_sfx.Basilisk_spit_acid2', [Pitch]0.75 );

	SetTimer(0.25,false);
}

function timer()
{
	//If you set the default properties, the 5 spells wont spawn on top of each other.  They screw up.
	SetCollisionSize( 20, 20 );
}

function OnSpellShutdown()
{
	// Clean up
	if( fxHeadParticleEffect != None )			
		fxHeadParticleEffect.Shutdown();
}

function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
{
	return true;
}

function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	// *****This spell is directed at Harry
	// *****Harry is not derived from HPawn so there is no HandleSpell function
	
}

// function OnPlaySpellCastSound()

//function PlayIncantationSound( actor Instigator, bool bSneaking )
//{
//	// we don't have a sound for this yet
//}


function getTarget()
{

}

function float getTime()
{
	local float t;
	local float distance;

	distance = vSize(location - playerHarry.location);

	t = (distance / fIncreaseHitTimeDistance) * fHitTimeIncrement;

//	playerHarry.clientMessage("Distance to harry : " $distance);

	return t;

}


function PlayerCutCapture()
{
	gotoState('CutIdle');
}



// --------------------------------------------------------------------------------------------
// *** States

state CutIdle
{

	begin:

	SetTimer(0,false);
	OnSpellShutdown();		
	Destroy();

}

state auto StateIdle
{
	begin:
	GotoState('StateFlying');
}

state StateFlying
{

	function BeginState()
	{

	//	Velocity = ComputeTrajectoryByTime( location, hitTarget, 2.0 );
		//Velocity = ComputeTrajectoryByTime( location, hitTarget, 1.0 );
		Velocity = ComputeTrajectoryByTime( location, hitTarget, 0.75 );
		loopAnim('idle');
	}


	// If the web hits harry a web will grow at his feet 
	function ProcessTouch(Actor Other, vector HitLocation)
	{

		if ( !other.IsA('SpiderMarker') && !other.IsA('LargeSpider') && !other.IsA('SpellWeb'))
		{
			Aragog(owner).createWeb(OldLocation);
		//	web = spawn(class'AragogStickyWeb',owner,,OldLocation,rot(0,0,0));
		//	web.fLifetime = fLifetime;

			if ( other.IsA('Harry') )
			{
				playerHarry.TakeDamage (iDamage, Pawn(Owner), location, velocity*1, 'AragogSpellAttack');
			}

			OnSpellShutdown();	
			Destroy();
		}
	}

	// The  has been changed to PHYS_Falling to use the new trajectory function. No longer hits walls
	// now only gets landed
	function Landed(vector HitNormal)
	{

		Aragog(owner).createWeb(OldLocation);
		//web = spawn(class'AragogStickyWeb',owner,,OldLocation,rot(0,0,0));
		//web.fLifetime = fLifetime;

		OnSpellShutdown();	
		Destroy();

	}

	// If the web spell hits a wall just destroy it in a puff of smoke (done by baseSpell)
	function HitWall(vector HitNormal, actor HitWall)
	{
 
		Super.HitWall( HitNormal, HitWall );

		OnSpellShutdown();	
		Destroy();

	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );
				
		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );
		}

		// Update our head particles
		if( fxHeadParticleEffect != None )
		{
			fxHeadParticleEffect.SetLocation( location );
		}
	}


	begin:
		loop:
		sleep(1);
		goto 'loop';
}


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{

	// --- ParticleFX
	fxFlyParticleEffectClass=class'AragogAttackFx'
	//fxHeadParticleEffectClass=class'Crabfireball'
        //fxHeadParticleEffectClass=class'AragogAttack'
	fxHitParticleEffectClass=class'SmokeExplo_01'
	Mesh=SkeletalMesh'HPModels.skAragogAttackMesh'
	
	// --- Base Spell
	spellType=SPELL_Web
	Physics=PHYS_Falling
	
    DrawType=DT_Mesh
	fIncreaseHitTimeDistance=150
	fHitTimeIncrement=0.5

//	CollisionRadius=20
//	CollisionHeight=20

	bCollideActors=true
	bCollideWorld=true
	bBlockActors=false
	bBlockPlayers=false
}

// --------------------------------------------------------------------------------------------
// spellweb.uc - End of file   
// --------------------------------------------------------------------------------------------