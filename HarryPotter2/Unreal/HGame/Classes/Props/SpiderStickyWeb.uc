class SpiderStickyWeb extends Characters;


// --------------------------------------------------------------------------------------------
// *** Variables


// Settings

// These were exposed to the editor in ectoplasma. Can add them to spellWeb if necessary
var float				fShrinkTime;			// how long will it take to shrink down to 0.0
var float				fGrowTime;				// how long will it take to grow
var sound				ShrinkSound;			// sound the web makes when shrinking
var sound				BumpSound;				// sound the web makes when something bumps into it
var float				grownDrawScale;			// the draw scale of the grown object


var float				fTimeSpent;
var actor				aSlimedHPawn;			// save the HPawn we are sliming

var float				fxParticlesPerSecond;	// how many particles per second do we want to produce
var ParticleFX			fxHit;					// ParticleFX hit ref
var ParticleFX			fxReact;				// ParticleFX react ref
var class<ParticleFX>	fxHitClass;				// class to create hit FX with
var class<ParticleFX>	fxReactClass;			// class to create react FX with

var float fWebLifetime;

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
}

function PostBeginPlay()
{
	Super.PostBeginPlay();
}

function Timer()
{
	playerHarry.clientMessage("The timer has fired");
	GotoState('stateHiding');
}

function Bump( actor other )
{
	PlaySound( BumpSound, SLOT_None );
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


function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{	
	// create the Rictusempra reaction
//	fxHit = spawn( fxHitClass );	
//	fxHit.SetLocation( location );
//	fxHit.SetOwner( Self );		
		
//	fxReact = spawn( fxReactClass );
//	fxReact.SetLocation( location );
//	fxReact.SetOwner( Self );
		
	// goto the hiding state
	GotoState('stateHiding');
		
	return true;
}

function UpdateFX()
{
	local vector colRotated;
	
	// rotate our collision h,w,d so that it corisponds with rotated web
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


// --------------------------------------------------------------------------------------------
// *** States


state auto stateIdle
{
	begin:

	// Start the timer to destroy the web
	SetTimer(fWebLifetime,false);

	// update the number of webs that this webs owner has out there
	SpiderLarge(owner).AddWebs();

	GotoState('stateShowing');
}


state() stateShowing
{

	function BeginState()
	{
		bCollideWorld		= true;
		eVulnerableToSpell	= SPELL_Rictusempra;
		fTimeSpent			= 0.0f;
/*
		// start the ambient sound
		if( AmbientSound != none )
		{
			// We need to loop this ambientSound
			AmbientSound = default.AmbientSound;
		}
*/
	}
	
	function Tick( float fTimeDelta )
	{
		// If we need to grow then do so
//		if( DrawScale < GetDefaultDrawScale() )
		if( DrawScale < grownDrawScale )
		{
			// update how much time has been spent
			fTimeSpent += fTimeDelta;
			
			// set our drawScale according to our shrinkTime
			DrawScale = (fTimeSpent / fGrowTime);
			
			if( DrawScale >= grownDrawScale )
			{
				// set our DrawScale to normal size
				DrawScale = grownDrawScale;
			}

			// update our particle FX
//			UpdateFX();
		}
	}

	function Touch( actor other )
	{		
		// If harry touches the web then he gets stuck.
		if( other.IsA('Harry') )
		{
			GotoState('StuckinSlime');
		
		}
	}
	
	function UnTouch( actor other )
	{
		// If harry Untouches the web then he is free to shoot spiders.
		if( other.IsA('Harry') )
		{
			Harry(other).WebAnimRefCountSub();
			aSlimedHPawn = none;
		}
	}
	
	begin:

//	LoopAnim( animsequence );
//	AnimFrame = RandRange( 0, 0.95 );
}

// Plays the correct anim. Starts the stuck in web animations, sets Harry as a SlimedHPawn and returns
// to stateShowing
state StuckinSlime
{

	begin:

	playerHarry.PlayAnim('webstuck');
	FinishAnim();

	playerHarry.WebAnimRefCountAdd();
	aSlimedHPawn = playerHarry;

	GotoState('stateShowing');

}

state() stateHiding
{
	function BeginState()
	{
		bCollideWorld		= false;
		eVulnerableToSpell	= SPELL_None;
		fTimeSpent			= 0.0f;
	
		// Just in case we were still sliming harry 
		// He can only cast Rictusempra while inside the web (cuz he's stuck)
		if( aSlimedHPawn != None && aSlimedHPawn.IsA('Harry') )
		{
			Harry(aSlimedHPawn).WebAnimRefCountSub();
		}
		aSlimedHPawn = none;

		PlaySound( ShrinkSound, SLOT_None );
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
//				fxHit.Shutdown();
//				fxReact.Shutdown();
/*			
				// stop the sound if it is playing
				if( AmbientSound != none )
				{
					AmbientSound = None;// we need to stop a looping ambient sound
				}
*/
				gotoState('stateDestroy');
			}
			
			// update our particle FX
//			UpdateFX();
		}
			
	}
/*
	function Touch( actor other )
	{
		// if its a ghost then show again?
	}
*/
	begin:
}

state stateDestroy
{
	begin:

	playerHarry.clientMessage(self.name $" :  We are in state Destroy");

	// Now that this is destroyed update the number of webs that this spider has out there
	SpiderLarge(owner).SubWebs();

	// since this is a web and NOT ectoplasma (even though right now it looks JUST like it
	// destroy the object. 
	Destroy();

}


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- SpiderStickyWeb

	fShrinkTime=1.7f
	fGrowTime=2.5f
	
	ShrinkSound=sound'HPSounds.Ch2Skurge.ecto_hit'
	BumpSound=sound'HPSounds.Ch2Skurge.ecto_blocking_movement'

	drawScale=0.2

	fxParticlesPerSecond=40

	fxHitClass=class'skurge_hit'
	fxReactClass=class'skurge_react'
	
	// --- Character
	eVulnerableToSpell=SPELL_Rictusempra
	
//	AmbientSound=Sound'HPSounds.Ch2Skurge.ecto_idle2'

	Physics=PHYS_None
	AnimSequence=idle1
	Style=STY_Translucent
	Texture=IceTexture'HPParticle.hp_fx.General.EctoplasmFX'
	Mesh=SkeletalMesh'HPModels.skectoplasmaMesh'
	AmbientGlow=255
	bUnlit=True
	bRandomFrame=True
	bMeshEnviroMap=True
	CollisionRadius=35
	CollisionHeight=10
	bBlockActors=False
	bBlockPlayers=False
	bBlockCamera=false

	grownDrawScale=1

	bDoEyeBlinks=false
}