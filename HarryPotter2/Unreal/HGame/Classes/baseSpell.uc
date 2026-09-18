// --------------------------------------------------------------------------------------------
//  _                     _____            _ _                
// | |                   / ____|          | | |               
// | |__   __ _ ___  ___| (___  _ __   ___| | |    _   _  ___ 
// | '_ \ / _` / __|/ _ \\___ \| '_ \ / _ \ | |   | | | |/ __|
// | |_) | (_| \__ \  __/____) | |_) |  __/ | | _ | |_| | (__ 
// |_.__/ \__,_|___/\___|_____/| .__/ \___|_|_|(_) \__,_|\___|
//                             | |                            
//                             |_|                            
// --------------------------------------------------------------------------------------------
// Class Name  : baseSpell
//
// Created on  : 03/29/2002
// 
// Description : The baseSpell class is the foundation for all spells
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class baseSpell extends Projectile;


#EXEC TEXTURE IMPORT NAME=defaultSpellIcon  FILE=TEXTURES\Menu\HUD\transSpellIcon.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


// --------------------------------------------------------------------------------------------
// *** Variables

// --- Base
var ESpellType	SpellType;		// enum type of spell
var texture		SpellIcon;		// icon of the spell
var float		SpellCharge;	// amount the spell has been charged. (between zero and one)
var float		SpellLifeTime;	// lifetime of spell

var baseWand	SpellWand;		// If this spell came from a wand then this will be set

// --- Target Info
var actor		TargetActor;	// Target actor the spell is seeking
var vector		TargetOffset;	// The offset from the target's location that will determine our dest point


// --- Seeking Info
var() float		SeekSpeed;		// SeekSpeed is how fast we wish to seek our target direction
var vector		CurrentDir;		// var used to keep track of our current smoothed out direction

// --- Visual FX

// Fly FX
var (VisualFX)ParticleFX		fxFlyParticleEffect;			
var (VisualFX)class<ParticleFX>	fxFlyParticleEffectClass;

// Hit FX
var (VisualFX)ParticleFX		fxHitParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHitParticleEffectClass;

var (VisualFX)ParticleFX		fxHitWallParticleEffect;
var (VisualFX)class<ParticleFX>	fxHitWallParticleEffectClass;	

// React FX
var (VisualFX)ParticleFX		fxReactParticleEffect;		
var (VisualFX)class<ParticleFX>	fxReactParticleEffectClass;



// --- Sound FX
var sound		CastSound;		
var string		SpellIncantation;	
var string		QuietSpellIncantation;


// --- Debug
var Harry		playerHarry;
var bool		bUseDebugMode;

// --------------------------------------------------------------------------------------------
// *** Constants

// --------------------------------------------------------------------------------------------
// *** Functions

function SetDebugMode( bool bOn )	{ bUseDebugMode = bOn; }

function InitSpell( Actor			  CastedBy, 
				    Actor			  CastedAt, 
					optional vector	  CastedAtOffset, 
					optional float	  CastedChargeAmount,
					optional baseWand CastedFromWand )
{
	local float scale;

	SetOwner( CastedBy );
	TargetActor		= CastedAt;
	TargetOffset	= CastedAtOffset;
	SpellWand		= CastedFromWand;
	
	// Create our flying particleFX
	if( fxFlyParticleEffect == None && fxFlyParticleEffectClass != None )
	{
		fxFlyParticleEffect = spawn( fxFlyParticleEffectClass );
		fxFlyParticleEffect.SetLocation( Location );
		fxFlyParticleEffect.SetRotation( Rotation );
	}
	
	// Scale our fly particles if there is a ChargeAmount
	SetSpellCharge( CastedChargeAmount );	
		
	if( bUseDebugMode )
		playerHarry.clientmessage("InitSpell: " $self
								 $" owner: "  $owner 
								 $" target: " $TargetActor 
								 $" charge: " $SpellCharge
								 $" speed: "  $speed );

	// Announce SpellInit
	OnSpellInit();
}

simulated function PostBeginPlay()
{
	local float scale;

	Super.PostBeginPlay();
	
	//Always do this, so the other spells can use playerHarry.
	playerHarry = Harry(Level.playerHarryActor);
			
	//Init our current seeking direction
	CurrentDir = vector(rotation);
}

event Destroyed()
{
	// Let our owner's wand know that we have been destroyed
	if( SpellWand != None )
		SpellWand.SubtractFromCastedSpellList( self );
	
	// Clean up
	if( fxFlyParticleEffect != None )			
		fxFlyParticleEffect.Shutdown();

	// We won't shutdown hitParticleEffect or ReactParticleEffect because they should die over time anyway.

	// We need to shutdown anything related to the traveling spell
	OnSpellShutdown();
}

function OnSpellInit()
{
	// implemented in derived class
}

function OnSpellShutdown()
{
	// implemented in derived class
}

event FellOutOfWorld()
{
	// over ride the actors version so we don't destroy ourselves
}

function PlayIncantationSound( actor Instigator )
{
	if( Instigator.IsA('Harry') )
		Harry(Instigator).HandleSpellIncantationSound( SpellType );
	else if( Instigator.IsA('HPawn') )
		HPawn(Instigator).HandleSpellIncantationSound( SpellType );
}

event Tick( float fTimeDelta )
{
	// Check to see if we should die
	if( (SpellLifeTime -= fTimeDelta) < 0 )
	{
		if( bUseDebugMode ) playerHarry.ClientMessage("Spell " $self $" LifeTime is up!" );

		// Destroy our spell
		OnSpellShutdown();
		Destroy();
	}
}

// --- OnHit functions

function bool OnSpellHitHarry( Actor aHit, vector HitLocation )
{
	return false; // Default is NOT valid
}

function bool OnSpellHitHPawn( Actor aHit, vector HitLocation )
{
	return false; // Default is NOT valid
}

function bool OnSpellHitWall( Actor aWall, vector HitNormal )
{
	// Create some dust
	fxHitWallParticleEffect = spawn( fxHitWallParticleEffectClass, [SpawnOwner]self, [SpawnLocation]location );	
	return true; // Default is valid
}



simulated function HitWall( vector HitNormal, actor Wall )
{
	// If we hit a wall then createHitEffects, shutdown then destroy
	if( Wall.IsA('GridMover') )
	{
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" HitWall GridMover: " $wall );
		CreateHitEffects( Wall, Location );
	}
	else
	{
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" HitWall Other: " $wall );
		
		// Let the spell do any "hitWall" specific code
		if( false == OnSpellHitWall( Wall, HitNormal ) )
			return; // don't do anything as hitting this wall is not a valid hit.
	}
		
	// Destroy our spell now that we hit something
	OnSpellShutdown();
	Destroy();
}

function ProcessTouch( Actor Other, vector HitLocation )
{
	// *** Reject invalid objects
//	if(bUseDebugMode) playerHarry.clientMessage("Spell::ProcessTouch : " $self $" other :" $other );
	
	// If we hit our owner and it wasn't our target Actor then we have an invalid touch.
	// also if we hit another spell or particle effect we have an invalid touch.
	if( Other == Owner || Other.IsA('baseSpell') || Other.IsA('ParticleFX') )
	{
		// we hit an invalid actor so keep moving
		// DEBUG
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" *INVALID* Touch to :" $other $"will not die yet.");
		return;
	}
	else if( Other.IsA('Harry') )
	{
		// OnSpellHitHarry will return false if the spell is not relevant to Harry
		if( false == OnSpellHitHarry( Other, HitLocation ) )
		{
			if(bUseDebugMode) playerHarry.clientMessage("Spell:" $self.Name $" *INVALID* Touch to Harry:" $other.Name $" NOT RELEVANT, OnSpellHitHarry() returned false!");
			return; //The spell will continue (with no hitEffects applied)
		}
		
		// We have a VALID hit
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" VALID Touch to Harry:" $other $" SpellCharge: " $SpellCharge );		
		CreateHitEffects( Other, HitLocation );
	}
	else if( Other.IsA('HPawn') )
	{
		// OnSpellHitHPawn will return false if the spell is not relevant to this hpawn
		if( false == OnSpellHitHPawn( Other, HitLocation ) )
		{
			if(bUseDebugMode) playerHarry.clientMessage("Spell:" $self.Name $" *INVALID* Touch to HPAWN:" $other.Name $" NOT RELEVANT, OnSpellHitHPawn() returned false!");
			return; //The spell will continue (with no hitEffects applied)
		}
		
		// We have a VALID hit
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" VALID Touch to HPAWN:" $other $" SpellCharge: " $SpellCharge );	
		HPawn(Other).OnSpellHit( self, HitLocation );		
		CreateHitEffects( Other, HitLocation );
	}
	else if( Other.IsA('spellTrigger') )
	{
		// DEBUG
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" VALID Touch to spellTrigger:" $other);
		CreateHitEffects( Other, HitLocation );
	}
	else
	{
		// DEBUG
		if(bUseDebugMode) playerHarry.clientMessage("Spell: " $self $" Touched ***UNCLASSIFIED***:" $other);
	}

	// reset physics to none, to allow engine to reset Touching array, 
	// to fix spelling bug, when after a few spells (4 - 5), nothing happens
	SetPhysics(PHYS_none);

	// We need to shutdown anything related to the traveling spell
	OnSpellShutdown();

	// Destroy our spell now that we hit something
	Destroy();
}

function static Texture GetSpellIcon()
{
	return default.SpellIcon;
}


// by default all spells are relevent to movers
function bool IsRelevantToMover()
{
	return true;
}

function ScaleParticles( ParticleFX FX, float scale )
{
	FX.ParticlesPerSec.Base		= FX.default.ParticlesPerSec.Base	* scale;
	FX.SourceHeight.Base		= FX.default.SourceHeight.Base		* scale;
	FX.SourceWidth.Base			= FX.default.SourceWidth.Base		* scale;
	FX.SourceDepth.Base			= FX.default.SourceDepth.Base		* scale;
	FX.SizeWidth.Base			= FX.default.SizeWidth.Base			* scale;
	FX.SizeLength.Base			= FX.default.SizeLength.Base		* scale;
	FX.AngularSpreadWidth.Base	= FX.default.AngularSpreadWidth.Base* scale;
    FX.AngularSpreadHeight.Base	= FX.default.AngularSpreadHeight.Base*scale;
	FX.SpinRate.Base			= FX.default.SpinRate.Base			* scale;
}

function CreateHitEffects( actor ActorHit, vector vHitLocation )
{
	local float scale;
	local float lTime;

	// Play Hit Sound Effects
	if( ImpactSound != None )
		PlaySound( ImpactSound, SLOT_None,  1.0, false, 2000.0, 1);
	
	//DEBUG
	if( bUseDebugMode ) 
		playerPawn(Instigator).clientMessage("Spell::CreateHitEffects using hitFXClass = " $fxHitParticleEffectClass $" reactFXClass = " $fxReactParticleEffectClass );

	// Spawn HitParticle Effects
	if( fxHitParticleEffectClass != None )
	{
		fxHitParticleEffect = spawn(fxHitParticleEffectClass );	
		fxHitParticleEffect.SetLocation( vHitLocation );
		fxHitParticleEffect.SetRotation( fxHitParticleEffect.default.rotation );
		fxHitParticleEffect.SetOwner( ActorHit );

		// this is done to keep hit spell alive for a while and move it with hit actor
		if(fxHitParticleEffect.IsA('duelRictusempra_hit') || fxHitParticleEffect.IsA('duelMimbleWimble_hit'))
		{
			if(fxHitParticleEffect.IsA('duelRictusempra_hit'))
			{
				duelRictusempra_hit(fxHitParticleEffect).HitActor = ActorHit;
			}
			else if(fxHitParticleEffect.IsA('duelMimbleWimble_hit'))
			{
				duelMimbleWimble_hit(fxHitParticleEffect).HitActor = ActorHit;
			}

			if(ActorHit.IsA('Harry'))
				lTime = playerHarry.fTimeAfterHit;
			else if	(ActorHit.IsA('Duellist'))
				lTime = Duellist(playerHarry.DuelOpponent).fTimeAfterHit;

			fxHitParticleEffect.LifeTime.Base = max(1.0, lTime);
		}
		
		// Scale our fly particles if there is a ChargeAmount
		if( SpellCharge > 0 && SpellWand != None )
		{
			ScaleParticles(fxHitParticleEffect, SpellWand.GetChargeParticleFXScale( SpellCharge ) );
		}
		
		//DEBUG
//		if( bUseDebugMode ) playerPawn(Instigator).ClientMessage("Created HitFX: " $fxHitParticleEffect $" loc:" $fxHitParticleEffect.Location $" rot:" $fxHitParticleEffect.Rotation );
	}
	
	// Spwan React Particle Effects
	if( fxReactParticleEffectClass != None )
	{
		fxReactParticleEffect = spawn( fxReactParticleEffectClass );
		fxReactParticleEffect.SetLocation( vHitLocation );
		fxReactParticleEffect.SetRotation( fxHitParticleEffect.default.rotation );
		fxReactParticleEffect.SetOwner( ActorHit );
		fxReactParticleEffect.SourceWidth.Base = HProp(ActorHit).collisionRadius;
		//DEBUG
//		if( bUseDebugMode ) playerPawn(Instigator).ClientMessage("Created ReactFX loc:" $fxReactParticleEffect.Location $" rot:" $fxReactParticleEffect.Rotation );
	}
}


//******************************
//*** Spell helper functions ***
//******************************

function SetSpellDirection( vector dir )
{
	CurrentDir	    = normal(dir);
	DesiredRotation = rotator(CurrentDir);
	SetRotation( DesiredRotation );
	fxFlyParticleEffect.SetRotation( DesiredRotation );
}

function vector GetTargetHitLocation()	
{ 
	return TargetActor.location + TargetOffset;
}

// Function that can be used by spells that are derived from baseSpell
function UpdateRotationWithSeeking( float fTimeDelta )
{
	local vector TargetDir;
	
	if( TargetActor == None )
		return;

	//find our target
	TargetDir = normal( GetTargetHitLocation() - location );
	
	// seek our target
	CurrentDir += (TargetDir - CurrentDir ) * FMin( 1.0f, SeekSpeed * fTimeDelta );
	CurrentDir = normal(CurrentDir);
	
	// set our current rotation
	DesiredRotation = rotator(CurrentDir);
	SetRotation( DesiredRotation );
	Velocity	 = CurrentDir * Speed;
}

function SetSpellCharge( float fNewCharge )
{
	local float scale;

	SpellCharge	= fNewCharge;
	
	// Scale our fly particles if there is a ChargeAmount
	if( SpellCharge > 0 && SpellWand != None )
	{
		ScaleParticles( fxFlyParticleEffect, SpellWand.GetChargeParticleFXScale( SpellCharge ) );
	}
}

function Reflect( actor aNewOwner, float fNewCharge, float fNewSpeed )
{
	local Pawn pawnOwner;
	
	//DEBUG
//	playerHarry.clientmessage("Original owner list before reflect swapping");
//	SpellWand.ShowCastedSpellList( 4 );

	// --- Swap our old owner properties with our new owner
	// Swap the spellWand we came from
	if( SpellWand != None )
		SpellWand.SubtractFromCastedSpellList( self );
	
	if( aNewOwner.IsA('Pawn') )
	{
		pawnOwner = pawn(aNewOwner);

		if( pawnOwner.weapon.IsA('baseWand') )
		{
			// Set the new spell wand we came from
			SpellWand = baseWand( pawnOwner.weapon );
			SpellWand.AddToCastedSpellList( self );
		}
	}

	//DEBUG
//	playerHarry.clientmessage("Original owner list *after* reflect swapping");
//	pawnOwner = Pawn(Owner);
//	baseWand(pawnOwner.weapon).ShowCastedSpellList( 4 );	
//	playerHarry.clientmessage("New owner list *after* reflect swapping");
//	SpellWand.ShowCastedSpellList( 4 );

	// Swap owners
	TargetActor	= Owner;
	SetOwner( aNewOwner );
	
	// --- Set New Charge
	SetSpellCharge( fNewCharge );
	
	// --- Set New Direction
	SetSpellDirection( GetTargetHitLocation() - location );
	
	// --- Set New Speed
	// set our current rotation
	Speed		= fNewSpeed;
	Velocity	= CurrentDir * Speed;
	
	// --- Reset our lifetime
	SpellLifeTime = default.SpellLifeTime;
	LifeSpan = default.LifeSpan;

	//DEBUG
	if( bUseDebugMode )
	playerHarry.ClientMessage( "*Spell REFLECTED by " $aNewOwner $", new owner = " $owner
					   $" new target: " $TargetActor 
					   $" new charge: " $SpellCharge
					   $" new speed: "  $speed );
}

// --------------------------------------------------------------------------------------------
// *** States -> (to be defined in the derived class )


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Base Spell
	bUseDebugMode=false
	spellType=SPELL_None
	spellIcon=Texture'defaultSpellIcon'

	SpellLifeTime=8.0

	CastSound=none

	SpellIncantation=""
	QuietSpellIncantation=""
	
	fxHitWallParticleEffectClass=Class'HPParticle.DustCloud02_small'
	
	SeekSpeed=7.0f

	bCollideActors=true
	bCollideWorld=true
	bBlockActors=false
	bBlockPlayers=false
	
	// --- Projectile	
	Damage=5
	Speed=500.0000
	LifeSpan=10.000000
	MomentumTransfer=0
	ImpactSound=Sound'HPSounds.magic_sfx.spell_hit'
	RemoteRole=ROLE_SimulatedProxy
	
	Mesh=Mesh'spellProj'
	
	bProjTarget=true

	bStatic=false
	style=sty_translucent   
	LightType=LT_Steady
	LightEffect=LE_NonIncidence
	LightBrightness=201
	LightHue=165
	LightSaturation=72
	LightRadius=10
	DrawScale=0.3  
	bUnlit=True
	bMeshCurvy=False
	CollisionRadius=2.0f
	CollisionHeight=2.0f
	bFixedRotationDir=True
	bNetTemporary=false
}

// --------------------------------------------------------------------------------------------
// baseSpell.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------