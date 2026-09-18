// --------------------------------------------------------------------------------------------
//                 _ _ 
//                | | |
//  ___ _ __   ___| | |
// / __| '_ \ / _ \ | |SnakeHeadFire.uc
// \__ \ |_) |  __/ | |
// |___/ .__/ \___|_|_|
//     | |             
//     |_|             
// --------------------------------------------------------------------------------------------
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellSnakeHeadFire extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	InitSpell(none,none);
}

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

auto state StateFlying
{
	function BeginState()
	{
		Velocity = vector(Rotation) * Speed;
	}

	function ProcessTouch(Actor Other, vector HitLocation)
	{
		if( pawn(other) == instigator )
			return;

		if( !other.bBlockActors )//spellSnakeHeadFire(other) != none )
			return;
	
		if( other.IsA('Harry') )
		{
			other.TakeDamage( Damage, instigator, vect(0,0,0), vect(0,0,0), '' );
			CreateHitEffects( Other, HitLocation );
		}

		OnSpellShutdown();
		
		Destroy();
	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );

//		UpdateRotationWithSeeking( fTimeDelta );
		
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
	// --- Spell Flipendo
	SeekSpeed=50.0f
	fxFlyParticleEffectClass=class'TorchFire04'

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

	Damage=10

	CollisionRadius=30
	CollisionHeight=30
}

// --------------------------------------------------------------------------------------------
// spellFlipendo.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


