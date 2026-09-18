
// Class Name  : spellFire
//
// Created on  : 04/18/2002
// Authored by : Janet Weddle
// 
// Description : The spellFire class implements the Fire spell specific code
//				 Harry can not cast Fire, instead the Firecrabs cast at Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class spellFire extends baseSpell;


// --------------------------------------------------------------------------------------------
// *** Variables


enum enumTargetZone
{
	ZONE_ONE,
	ZONE_TWO,
	ZONE_THREE,
	ZONE_FOUR
};

var enumTargetZone eTargetZone;

var float	fGravityEffect;
var vector	CurrentDir;	
var vector	savedVelocity;

var bool	bBounce;
var vector	currentVelocity;


// --------------------------------------------------------------------------------------------
// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	// Init our seeking current Dir var
	CurrentDir = normal(vector(Rotation) + vec(0,0,0.4f) );
	SetRotation( rotator( CurrentDir ) );
}

function timer()
{
	bBounce = false;
}

function OnSpellShutdown()
{
}



function bool OnSpellHitHPawn( Actor aHit, vector vHitLocation)
{
	// All HPawns have a HandleSpell function
	// *****This spell is directed at Harry
	// *****Harry is not derived from HPawn so there is no HandleSpell function
	
}


function bounce( vector HitNormal)
{

	// Get out of the wall or floor
	SetLocation( OldLocation );

	// Slow it down
	Velocity *= 0.90;

	// Set the velocity to reflect the normal
	Velocity = MirrorVectorByNormal( Velocity, HitNormal );

	// Set the rotation of the particleEffect. 
	fxFlyParticleEffect.SetRotation( rotator(velocity) );

	// Set our Direction vector
	CurrentDir = vector( Rotation );

	// Aim the Current Direction to the normal.
	CurrentDir += HitNormal;
	
	SetRotation( rotator( CurrentDir ) );

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
		currentVelocity = Velocity;
		if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("Begin Velocity =  " $ Velocity );
		SetTimer(5.00,false);
	}

	function ProcessTouch(Actor Other, vector HitLocation)
	{
		if ( bBounce == false )
		{
			if( pawn(other) == instigator )
				return;
		
			if( other.IsA('Harry') )
			{
				other.TakeDamage( 5, instigator, vect(0,0,0), vect(0,0,0), '' );
				CreateHitEffects( Other, HitLocation );
			}
			else
			{
				spawn(class'fireball',,,Location);
			}
		
			OnSpellShutdown();
			
			Destroy();
		}
		else
		{
			if( pawn(other) == instigator )
			{
				OnSpellShutdown();
				Destroy();
			}

			if( other.IsA('Harry') )
			{
				other.TakeDamage( 10, instigator, vect(0,0,0), vect(0,0,0), '' );
				CreateHitEffects( Other, HitLocation );
				OnSpellShutdown();
				Destroy();
			}

		}
		
	}

	function HitWall(vector HitNormal, actor HitWall)
	{
		if ( bBounce == false )
		{
			spawn(class'fireball',,,Location);
			Super.HitWall( HitNormal, HitWall );
		}
		else
		{
			bounce(HitNormal);
		}

	}

	event Tick( float fTimeDelta )
	{
		super.Tick( fTimeDelta );

	
		if ( bBounce == false )
		{
			velocity.z += -fGravityEffect * fTimeDelta;
		}
		else
		{
			velocity.z += (-fGravityEffect/2) * fTimeDelta;
		}
			
		// Update our fly particles
		if( fxFlyParticleEffect != None )
		{
			fxFlyParticleEffect.SetLocation( location );
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

	fxFlyParticleEffectClass=class'CrabFire'

	
	
	// --- Base Spell
	spellType=SPELL_Fire
	
//	SpellIncantation="spells3"
//	QuietSpellIncantation="spells4"
	
	fxHitParticleEffectClass=class'SmokeExplo_01'
	
	// --- ParticleFX
	Speed=300.0f
    DrawType=DT_None

	fGravityEffect=200	// seems really high but I like the way it looks
	bBounce=True

}

// --------------------------------------------------------------------------------------------
// spellFire.uc - End of file   
// --------------------------------------------------------------------------------------------