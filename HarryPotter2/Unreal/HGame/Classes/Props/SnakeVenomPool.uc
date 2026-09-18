//===============================================================================
//  [Basilisk venom mesh] 
//===============================================================================

class SnakeVenomPool extends Hprop;

// --------------------------------------------------------------------------------------------
// *** Variables

var() float				fShrinkTime;			// how long will it take to shrink down to 0.0
var() float				fGrowTime;				// how long will it take to grow

var() float 			fDamageTimer;			// how often to we cause damage to harry
var() int				iDamage;				// how much damage do we cause to harry
var   bool				bGrowOnEvent;			// Grow on event? else Shrink on event

var float				fTimeSpent;
var actor				aSlimedHPawn;			// save the HPawn we are sliming

var() sound				ShrinkSound;

var float				fxParticlesPerSecond;	// how many particles per second do we want to produce
//var ParticleFX			fxHit;					// ParticleFX hit ref
//var ParticleFX			fxReact;				// ParticleFX react ref
//var class<ParticleFX>	fxHitClass;				// class to create hit FX with
//var class<ParticleFX>	fxReactClass;			// class to create react FX with

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PreBeginPlay()
{
	Super.PreBeginPlay();

	if( !bInCurrentGameState )
	{
		// We are NOT in the current state
		bHidden = true;
		SetCollision(false,false,false);
		return;
	}

	// Set our collision to collide actors = true, block actors or players == false
	SetCollision( true, false, false );

	// Find harry and save him
	//foreach AllActors( class'Harry', playerHarry )
	//	if( playerHarry.bIsPlayer && playerHarry != Self )
	//		break;

	// Setup a timer for how oftin we cause damage to harry (if we are touching him)
	SetTimer(fDamageTimer, true);
	
	if( InitialState == 'stateHiding' )
	{
		// don't wait for the hiding state to shrink the ecto and hide
		DrawScale = 0.0f;
		bHidden   = true;
	}

	fShrinkTime *= RandRange(0.8,1.2);
	fGrowTime   *= RandRange(0.8,1.2);

	if( FRand() > 0.75 )
	{
		switch( Rand(6) )
		{
			case 0:   PlaySound( sound'HPSounds.Adv11_cos.ss_COS_venomland_01E', [Volume]RandRange(0.3,0.6), [Pitch]RandRange(0.8,1.2) );   break;
			case 1:   PlaySound( sound'HPSounds.Adv11_cos.ss_COS_venomland_05E', [Volume]RandRange(0.3,0.6), [Pitch]RandRange(0.8,1.2) );   break;
			case 2:   PlaySound( sound'HPSounds.Adv11_cos.ss_COS_venomland_04E', [Volume]RandRange(0.3,0.6), [Pitch]RandRange(0.8,1.2) );   break;
			case 3:   PlaySound( sound'HPSounds.Adv11_cos.ss_COS_venomland_03E', [Volume]RandRange(0.3,0.6), [Pitch]RandRange(0.8,1.2) );   break;
			case 4:   PlaySound( sound'HPSounds.Adv11_cos.ss_COS_venomland_02E', [Volume]RandRange(0.3,0.6), [Pitch]RandRange(0.8,1.2) );   break;
			case 5:   PlaySound( sound'HPSounds.Adv11_cos.ss_COS_venomland_06E', [Volume]RandRange(0.3,0.6), [Pitch]RandRange(0.8,1.2) );   break;
		}
	}
}

function float GetDefaultDrawScale()
{
	// this function is to get around a bug where if you use "default.DrawScale" 
	// it will use the parent's if it is used in the parents function, 
	// not the derived class's version of default.DrawScale
	return default.DrawScale;
}

function UpdateFX()
{
	local vector colRotated;

  return;
/*
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
*/
}

// --------------------------------------------------------------------------------------------

simulated function Timer()
{
	//playerHarry.ClientMessage("playerHarry.z="$playerHarry.Location.z$" MyZ="$ Location.z$" MyDS="$DrawScale$" MyCS:"$CollisionRadius);

	// If we are touching harry then hurt him
	//if( aSlimedHPawn != none && aSlimedHPawn.IsA('Harry') )
	if(   Vsize2d(playerHarry.Location - Location)  <  playerHarry.CollisionRadius + CollisionRadius*DrawScale
	   && Abs( playerHarry.Location.z/*+playerHarry.FootOffsetZ*/ - Location.z )  <  playerHarry.CollisionHeight + CollisionHeight
	   && iDamage > 0  //need to have a damage amount greater than zero.
	  )
	{
		playerHarry.TakeDamage(iDamage, Self, location, vec(0,0,0) , 'ectoplasma' );
		
		//DEBUG
		playerHarry.ClientMessage("DamageTimer " $fDamageTimer $", taking damage " $iDamage $", current health is " $playerHarry.GetHealthStatusItem().nCount );
	}
}


// ------------------------------------------------------
auto state stateGrowing
{
	function BeginState()
	{
		DrawScale = 0;

		bCollideWorld		= true;			// we need to have the ectoplasma stop gridMovers
		fTimeSpent			= 0.0f;

		// start the ambient sound
		//if( AmbientSound != none )
		//{
		//	// We need to loop this ambientSound
		//	AmbientSound = default.AmbientSound;
		//	//playerHarry.ClientMessage("Started the Ecto Ambient Sound:" $AmbientSound $" fTimeSpent= " $fTimeSpent );
		//}
	}
	
	function Tick( float fTimeDelta )
	{
		// If we need to grow then do so
		if( DrawScale < GetDefaultDrawScale() )
		{
			// update how much time has been spent
			fTimeSpent += fTimeDelta;
			
			// set our drawScale according to our shrinkTime
			DrawScale = (fTimeSpent / fGrowTime) * GetDefaultDrawScale();
			
			if( DrawScale >= GetDefaultDrawScale() )
			{
				DrawScale = GetDefaultDrawScale();
				GotoState( 'stateShrinking' );
			}

			// update our particle FX
			UpdateFX();
		}
	}

  begin:
	LoopAnim( animsequence );
	AnimFrame = RandRange( 0, 0.9 );
}

// ------------------------------------------------------
state stateShrinking
{
	function BeginState()
	{
		//bCollideWorld		= false;
		//eVulnerableToSpell	= SPELL_None;
		fTimeSpent			= 0.0f;

		//PlaySound( ShrinkSound, SLOT_Misc );
	}
	
	function Tick( float fTimeDelta )
	{
		// update how much time has been spent
		fTimeSpent += fTimeDelta;
		
		// set our drawScale according to our shrinkTime
		if( fTimeSpent >= fShrinkTime )
		{
			//We're done
			Destroy();
		}
		else
		{
			DrawScale = (fShrinkTime - fTimeSpent)/fShrinkTime  *  GetDefaultDrawScale();

			// update our particle FX
			UpdateFX();
		}
	}

}


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Ectoplasma
	fDamageTimer=0.5
	iDamage=2

	fShrinkTime=17f
	fGrowTime=0.3f
	
	// --- Character
	eVulnerableToSpell=SPELL_None
	
	AmbientSound=none //Sound'HPSounds.Ch2Skurge.ecto_idle2'

	Physics=PHYS_None
	AnimSequence=idle1

     Mesh=SkeletalMesh'HProps.skvenomMesh'
     AmbientGlow=128
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.Venomwet'
     CollisionRadius=44
     CollisionHeight=5
     bBlockActors=False
     bBlockPlayers=False


     Style=STY_Translucent
     Mesh=SkeletalMesh'HProps.skvenomMesh'
     AmbientGlow=128
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.Venomwet'
     CollisionRadius=44
     CollisionHeight=5
     bBlockActors=False
     bBlockPlayers=False
	 bBlockCamera=false
}
