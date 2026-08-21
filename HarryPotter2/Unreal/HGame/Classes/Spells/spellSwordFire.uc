// --------------------------------------------------------------------------------------------
//                 _ _ 
//                | | |
//  ___ _ __   ___| | |
// / __| '_ \ / _ \ | |SwordFire.uc
// \__ \ |_) |  __/ | |
// |___/ .__/ \___|_|_|
//     | |             
//     |_|             
// --------------------------------------------------------------------------------------------
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellSwordFire extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables

var()   float  fNormalDamage;
var()   float  fFullDamage;
var()   float  fNormalScale;
var()   float  fFullScale;

//var     float  ParticlesPerSec;
var     float  fCurrentScale; //This one starts at 1 and goes to 0
var()   float  fHalfLife;
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
	//playerHarry.ClientMessage("*** Spell sword fire hit hpawn:"$aHit.name);
	if( aHit.IsA( 'GenericColObj' ) )
		return true;

	if( !aHit.bBlockActors )
		return false;

	return true;
	//if( aHit
	//return HPawn(aHit).HandleSpellFlipendo( vHitLocation );
}

function PlayIncantationSound( actor Instigator )
{
	//do nothing
}

function DamagePercent( float scale )
{
	local float scale2;

	Damage = fNormalDamage + (fFullDamage-fNormalDamage) * scale;	
//playerHarry.ClientMessage("*** Spell sword fire.  Damage:"$Damage);

	scale2 = fNormalScale + (fFullScale - fNormalScale) * scale;

	fxFlyParticleEffect.SizeWidth.Base       = fxFlyParticleEffect.default.SizeWidth.Base  * scale2;
	fxFlyParticleEffect.SizeLength.Base      = fxFlyParticleEffect.default.SizeLength.Base * scale2;
	fxFlyParticleEffect.ParticlesPerSec.Base = fxFlyParticleEffect.default.ParticlesPerSec.Base * scale2;

	fxFlyParticleEffect.SourceHeight.Base    = fxFlyParticleEffect.default.SourceHeight.Base * scale2;
	fxFlyParticleEffect.SourceWidth.Base     = fxFlyParticleEffect.default.SourceWidth.Base * scale2;
	fxFlyParticleEffect.SourceDepth.Base     = fxFlyParticleEffect.default.SourceDepth.Base * scale2;

	if( scale < 0.333 )
		PlaySound(sound'HPSounds.Magic_sfx.sword_shoot', SLOT_Interact);
	else
	if( scale < 0.667 )
		PlaySound(sound'HPSounds.Magic_sfx.sword_shoot_big', SLOT_Interact);
	else
		PlaySound(sound'HPSounds.Magic_sfx.sword_shoot_biggest', SLOT_Interact);
}


// --------------------------------------------------------------------------------------------
// *** States

auto state StateFlying
{
	function BeginState()
	{
		Velocity = vector(Rotation) * Speed;
	}

	event Tick( float fTimeDelta )
	{
		local float  scale;

		super.Tick( fTimeDelta );

		//UpdateRotationWithSeeking( fTimeDelta );
		
		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );

			if( false )
			{
				scale = 1.0 / Exp(fTimeDelta/fHalfLife); //2^(fTimeDelta/fHalfLife); of course there's no pow func.
				fCurrentScale *= scale;//fCurrentScale / 2^(fTimeDelta/fHalfLife);
				fxFlyParticleEffect.SizeWidth.Base       *= scale;
				fxFlyParticleEffect.SizeLength.Base      *= scale;
				fxFlyParticleEffect.ParticlesPerSec.Base *= scale;
				fxFlyParticleEffect.SourceHeight.Base    *= scale;
				fxFlyParticleEffect.SourceWidth.Base     *= scale;
				fxFlyParticleEffect.SourceDepth.Base     *= scale;

				fxFlyParticleEffect.AlphaStart.Base *= scale;
				fxFlyParticleEffect.AlphaEnd.Base   *= scale;

				Damage *= scale;
			}
		}
		//Log("*************** spell Damage:"$Damage);
	}
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spell Flipendo
	SeekSpeed=50.0f
	fxFlyParticleEffectClass=class'SwordFireBall'

	// --- Base Spell
	spellType=SPELL_Flipendo
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells1"
	QuietSpellIncantation="spells10"
	
	fxHitParticleEffectClass=class'flip_hit'
//	fxReactParticleEffectClass=class'flip_react'
	
	// --- ParticleFX
	Speed=500.0f
    DrawType=DT_None

	Damage=5
	fNormalDamage=3
	fFullDamage=23
	fNormalScale=0.5
	fFullScale=6

	fCurrentScale=1
	fHalfLife=0.75
}

// --------------------------------------------------------------------------------------------
// spellFlipendo.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


