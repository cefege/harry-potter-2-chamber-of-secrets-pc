// --------------------------------------------------------------------------------------------
//                 _ _ 
//                | | |
//  ___ _ __   ___| | |
// / __| '_ \ / _ \ | |AcidSpit.uc
// \__ \ |_) |  __/ | |
// |___/ .__/ \___|_|_|
//     | |             
//     |_|             
// --------------------------------------------------------------------------------------------
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellAcidSpit extends baseSpell;



// --------------------------------------------------------------------------------------------
// *** Variables

//var(VisualFX) ParticleFX        fxSmallBounceEffect;
//var(VisualFX) class<ParticleFX> fxSmallBounceEffectClass;

var   float                     FloorZ; //Spell will only land on this floor height.

var() int				        iDamage;	// how much damage do we cause to harry

var   bool                      bSpawnPool;

var   float                     fPoolShrinkTimeMult; //normally zero

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions

function PostBeginPlay()
{
	super.PostBeginPlay();
	InitSpell(none,none);
}

event BeginEvent()	{}
event EndEvent()	{}
event KilledBy( pawn EventInstigator )	{}

function OnSpellShutdown()
{
}

function HitWall(vector HitNormal, actor Wall)
{
	local AcidSpitPool  a;
	local Rotator       r;

	//playerHarry.clientMessage("********* Spell: " $self $" HitWall Other: " $wall );
	//playerHarry.clientMessage("**Z Val="$Location.Z-CollisionHeight/2 - FloorZ );

	//Bounce if we're not to FloorZ yet.
	if( (Location.Z-CollisionHeight/2 - FloorZ) > 10 )
	{
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );
		Velocity *= 0.25;
		//Velocity.z *= 0.333;
		if( VSize( Velocity ) < 100 )
			Velocity *= 100/VSize( Velocity );
	}
	else //Make the acid pool
	{
		//But only do it if it's supposed to
		if( bSpawnPool )
		{
			a = spawn(class'AcidSpitPool');
			r.yaw = Rand(65536);
			a.SetRotation( r );

			if( fPoolShrinkTimeMult > 0 )
				a.fShrinkTime *= fPoolShrinkTimeMult;
		}

		Destroy();
	}
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
		super.Tick( fTimeDelta );

		// Update our fly particles
		if( fxFlyParticleEffect != None )
			fxFlyParticleEffect.SetLocation( location );
	}

}

function ProcessTouch( Actor Other, vector HitLocation )
{
	// *** Reject invalid objects

	// If we hit our owner and it wasn't our target Actor then we have an invalid touch.
	// also if we hit another spell or particle effect we have an invalid touch.
	if( Other.IsA('Harry')  &&  iDamage > 0 )
	{
		playerHarry.TakeDamage(iDamage, none, location, vec(0,0,0) , 'ectoplasma' );
		CreateHitEffects( Other, HitLocation );
		Destroy();
		return;
	}

	super.ProcessTouch( Other, HitLocation );
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	fxFlyParticleEffectClass=class'SnakeVenomFX'

	Physics=PHYS_Falling

	ImpactSound=sound'HPsounds.Adv11_COS.ss_COS_venom_hit_Harry';

	// --- Base Spell
	spellType=SPELL_Flipendo
	spellIcon=Texture'alohoSpellIcon'
	
	SpellIncantation="spells1"
	QuietSpellIncantation="spells10"
	
	fxHitParticleEffectClass=class'flip_hit'
//	fxReactParticleEffectClass=class'flip_react'

	CollisionRadius=10
	CollisionHeight=10
	
	// --- ParticleFX
	Speed=1000.0f
    DrawType=DT_None

	bBounce=true

	bSpawnPool=true

	iDamage=25
}

// --------------------------------------------------------------------------------------------
// spellFlipendo.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------


