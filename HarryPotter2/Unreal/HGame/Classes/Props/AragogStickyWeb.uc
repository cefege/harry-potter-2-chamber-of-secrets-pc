class AragogStickyWeb extends Characters;


// --------------------------------------------------------------------------------------------
// *** Variables


// Settings

var float				fShrinkTime;			// how long will it take to shrink down to 0.0
var float				fGrowTime;				// how long will it take to grow
var sound				ShrinkSound;			// sound the web makes when shrinking
var float				grownDrawScale;			// the draw scale of the grown object


var float				fTimeSpent;
var actor				aSlimedHPawn;			// save the HPawn we are sliming

//var float				fxParticlesPerSecond;	// how many particles per second do we want to produce
var ParticleFX			fxHit;					// ParticleFX hit ref
var ParticleFX			fxReact;				// ParticleFX react ref
var class<ParticleFX>	fxHitClass;				// class to create hit FX with
var class<ParticleFX>	fxReactClass;			// class to create react FX with

var float			fLifetime;					// How long does this web last
var float 			fDamageTimer;				// how often to we cause damage to harry
var int				iDamage;					// how much damage to we cause Harry


var float			DamageTime;					// How much time is left till damage
var bool				bTouch;					// Once you touch the web you can't leave until destroyed 
												// so don't accept a second touch on the same web

// --------------------------------------------------------------------------------------------
// *** Constants

// --------------------------------------------------------------------------------------------
// *** Functions

function PreBeginPlay()
{
	Super.PreBeginPlay();

	// Set our collision to collide actors = true, block actors or players == false
	SetCollision( true, false, false );

//	DamageTime = fDamageTimer;
	DamageTime = 0;
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

function Timer()
{
	GotoState('stateHiding');
}

function BeginToGrow()
{
	PlaySound( ShrinkSound, SLOT_None, [volume]0.25);

	GotoState('stateShowing');
}


// --------------------------------------------------------------------------------------------
// *** States


state auto stateIdle
{
	begin:

	DrawScale = 0.0f;
	bHidden   = true;

}


state stateShowing
{

	function BeginState()
	{
		bCollideWorld		= true;
		eVulnerableToSpell	= SPELL_None;
		fTimeSpent			= 0.0f;
		bHidden				= false;

		if ( FastTrace(location + vec(0,0,-45), location) )
		{
			SetPhysics(PHYS_Falling);
		}

		fGrowTime *= RandRange( 0.2, 0.3 );
	}
	
	function Tick( float fTimeDelta )
	{
		Super.Tick(fTimeDelta);

		DamageTime -= fTimeDelta;

		if ( DamageTime <= 0 )
		{
			DamageTime = fDamageTimer;

			if( aSlimedHPawn != none && aSlimedHPawn.IsA('Harry') )
			{
				Harry(aSlimedHPawn).TakeDamage(iDamage, Self, location, vec(0,0,0) , 'AragogStickyWeb' );
			
				//DEBUG
				playerHarry.ClientMessage("DamageTimer " $fDamageTimer $", taking damage " $iDamage $", current health is " $playerHarry.GetHealthStatusItem().nCount );
			}
		}

		// If we need to grow then do so
		if( DrawScale < grownDrawScale )
		{
			// update how much time has been spent
			fTimeSpent += fTimeDelta;
			
			// set our drawScale according to our shrinkTime
			DrawScale = (fTimeSpent / fGrowTime);
			
			if( DrawScale >= grownDrawScale )
			{

				SetTimer( fLifetime + RandRange(-fLifetime/5,fLifetime/5), false);

				// set our DrawScale to normal size
				DrawScale = grownDrawScale;

				gotoState('DoneGrowing');
			}

		}
	}

	function Touch( actor other )
	{
		if ( bTouch == false )
		{
			bTouch = true;

			// If harry touches the web then he gets stuck.
			if( other.IsA('Harry') )
			{
playerHarry.clientMessage("touched while growing");
				Aragog(owner).bOnStickyWeb = true;
				playerHarry.WebAnimRefCountAdd();
				aSlimedHPawn = playerHarry;
				//GotoState('StuckinSlimeWhileGrowing');		
			}
		}
	}

	function Bump( actor other )
	{
		touch(other);
	}

	
	function UnTouch( actor other )
	{
		bTouch = false;
		// If harry Untouches the web then he is free to shoot spiders.
		if( other.IsA('Harry') )
		{
			Harry(other).WebAnimRefCountSub();
			aSlimedHPawn = none;
		}
	}

	
	begin:

}

// Sets Aragogs bOnStickyWeb boolean so Aragog will attack. 
// Increments the StickyWeb ref count in Harry, sets Harry as a SlimedHPawn and returns
// to stateShowing
state StuckinSlimeWhileGrowing
{

	begin:

	Aragog(owner).bOnStickyWeb = true;

	playerHarry.WebAnimRefCountAdd();
	aSlimedHPawn = playerHarry;

	GotoState('stateShowing');

}


state DoneGrowing
{
	function BeginState()
	{
		eVulnerableToSpell	= SPELL_Diffindo;
	}


	function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
	{		
		// goto the hiding state
		GotoState('stateHiding');
		
		return true;
	}

	function Touch( actor other )
	{
		if ( bTouch == false )
		{
			bTouch = true;

			// If harry touches the web then he gets stuck.
			if( other.IsA('Harry') )
			{
playerHarry.clientMessage("touched when DONE growing");
				Aragog(owner).bOnStickyWeb = true;
				playerHarry.WebAnimRefCountAdd();
				aSlimedHPawn = playerHarry;
				//GotoState('StuckinSlimeDoneGrowing');		
			}
		}
	}

	function Tick( float fTimeDelta )
	{
		Super.Tick(fTimeDelta);

		DamageTime -= fTimeDelta;

		if ( DamageTime <= 0 )
		{
			DamageTime = fDamageTimer;

			if( aSlimedHPawn != none && aSlimedHPawn.IsA('Harry') )
			{
				Harry(aSlimedHPawn).TakeDamage(iDamage, Self, location, vec(0,0,0) , 'AragogStickyWeb' );
			
				//DEBUG
				playerHarry.ClientMessage("DamageTimer " $fDamageTimer $", taking damage " $iDamage $", current health is " $playerHarry.GetHealthStatusItem().nCount );
			}
		}
	}

	function UnTouch( actor other )
	{
		bTouch = false;
		// If harry Untouches the web then he is free to shoot spiders.
		if( other.IsA('Harry') )
		{
			Harry(other).WebAnimRefCountSub();
			aSlimedHPawn = none;
		}
	}


	begin:

	LoopAnim( animsequence );
	AnimFrame = RandRange( 0, 0.95 );

}

// Sets Aragogs bOnStickyWeb boolean so Aragog will attack. 
// Increments the StickyWeb ref count in Harry, sets Harry as a SlimedHPawn and returns
// to DoneGrowing
state StuckinSlimeDoneGrowing
{

	begin:

	Aragog(owner).bOnStickyWeb = true;

	playerHarry.WebAnimRefCountAdd();
	aSlimedHPawn = playerHarry;

	GotoState('DoneGrowing');

}


state() stateHiding
{
	function BeginState()
	{
		bCollideWorld		= false;
		eVulnerableToSpell	= SPELL_None;
		fTimeSpent			= 0.0f;
	
		// Just in case we were still sliming harry 
		// He can only cast Diffindo while inside the web (cuz he's stuck)
		if( aSlimedHPawn != None && aSlimedHPawn.IsA('Harry') )
		{
			Harry(aSlimedHPawn).WebAnimRefCountSub();
		}
		aSlimedHPawn = none;

		fShrinkTime *= RandRange(0.8, 1.2);

		PlaySound( ShrinkSound, SLOT_None, [volume]0.25);
	}

	function Tick( float fTimeDelta )
	{
		// shrink then hide
		if( bHidden == false )
		{
			// update how much time has been spent
			fTimeSpent += fTimeDelta;
			
			// set our drawScale according to our shrinkTime
		//	DrawScale = GetDefaultDrawScale() - (fTimeSpent / fShrinkTime );
			DrawScale = grownDrawScale - (fTimeSpent / fShrinkTime );

			if( DrawScale <= 0.0f )
			{
				DrawScale = 0.0f;
				bHidden   = true;
				
				// stop our particle emitting
				if( fxHit != None )
					fxHit.Shutdown();
	
				if( fxReact != None )
					fxReact.Shutdown();
			}

		}
			
	}

	begin:
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- AragogStickyWeb
	fDamageTimer=0.05
	iDamage=2

	fShrinkTime=1.7f
	fGrowTime=0.25f
	fLifetime=1.0
	
	ShrinkSound=sound'HPSounds.Ch2Skurge.ecto_hit'

	drawScale=0.2

//	fxParticlesPerSecond=40

	fxHitClass=class'skurge_hit'
	fxReactClass=class'skurge_react'
	
	// --- Character
	eVulnerableToSpell=SPELL_Diffindo
	
	AmbientSound=Sound'HPSounds.Ch2Skurge.ecto_idle2'

	Physics=PHYS_None
	AnimSequence=idle
	//Style=STY_Translucent
	//Texture=IceTexture'HPParticle.hp_fx.General.EctoplasmFX'
	//Mesh=SkeletalMesh'HPModels.skectoplasmaMesh'
	Mesh=SkeletalMesh'HPModels.skAragogStickyWebMesh'
	//AmbientGlow=255
	//bUnlit=True
	bRandomFrame=True
	//bMeshEnviroMap=True
	CollisionRadius=25
	CollisionHeight=10
	bBlockActors=False
	bBlockPlayers=False
	bCollideWorld=True

	grownDrawScale=1

	bDoEyeBlinks=false
}