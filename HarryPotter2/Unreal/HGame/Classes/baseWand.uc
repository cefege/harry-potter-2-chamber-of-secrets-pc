// --------------------------------------------------------------------------------------------
//  _                    _   _   _                 _                
// | |                  | | | | | |               | |               
// | |__   __ _ ___  ___| | | | | | __ _ _ __   __| |    _   _  ___ 
// | '_ \ / _` / __|/ _ \ | | | | |/ _` | '_ \ / _` |   | | | |/ __|
// | |_) | (_| \__ \  __/  V _ V  | (_| | | | | (_| | _ | |_| | (__ 
// |_.__/ \__,_|___/\___|\__/ \__/ \__,_|_| |_|\__,_|(_) \__,_|\___|
//                                                                  
//                                                                  
// --------------------------------------------------------------------------------------------
// Class Name  : baseWand
//
// Created on  : 04/01/2002
// 
// Description : The baseWand class implements the main spell managment system for choosing a spell and casting one
// 
// 
// How to add a Spell :
// 
// Step 1:	Make sure the baseWand::SpellBook is big enough for the total number of spells for a wand.
//
// Step 2:	Add your new spell to "enum ESpellType" in the "actor" class.
//
// Step 3:	Add the correct refrence to your new spell in the following functions:
//				baseWand::AddAllSpells(), baseWand::ChooseSpell().
//
// Step 4:	Add a handler function for your new spell in the "HPawn" class, here is an example:
//			function bool HandleSpellFlipendo( vector vHitLocation )
//
// Step 5:  Finnaly create the new spell class and make sure it inherites from baseSpell.uc.
//	
// Test your new spell! Then when your done with that, Test it again! 
//		An easy way to test a spell is to overrite the ChooseSpell function call in baseWand::Cast()
//	
//
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class baseWand extends HWeapon;


// Lumos FX texture
#EXEC TEXTURE IMPORT NAME=defaultSpellIcon  FILE=TEXTURES\Menu\HUD\transSpellIcon.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=LumosLightIcon  FILE=TEXTURES\LumosLight.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

// --------------------------------------------------------------------------------------------
// *** Variables


// --- Spell Managment
var bool				bAutoSelectSpell;	// autoSelect a spell based upon target's vulnerability
var class<baseSpell>	CurrentSpell;		// current spell the wand is using

var float				fAutoHitDistance;	// if our target is close enough then have the casted spell autoHit it.

var baseSpell			CastedSpellList[8]; // array of spells that have been casted by this wand
var baseSpell			LastCastedSpell;	// last spell we casted
var int					NumCastedSpells;	// number of casted spells
const					MAX_NUM_CASTED_SPELLS = 8;

// --- Wand FX
var particleFX			fxChargeParticles;	// wand particles used when charging up for a cast
var class<particleFX>	fxChargeParticleFXClass;

// --- Lumos specific
var LumosLight			TheLumosLight;

// --- Instant Fire specific
var bool                bInstantFire;

// --- Spell Charging
var bool				bSpellCharges;			// does the current spell charge at all?
var float				fSpellCharge;			// number between zero and one. where one is fully charged
var float               fSpellChargeTime;       // time spent charging
var float               fSpellChargeTimeSpan;   // max time we can charge to
var float               fSpellChargeStartScale; // starting charge scale
var float               fSpellChargeEndScale;   // ending charge scale

// --- Using sword specific.  Should this be a separate weapon?  Probably should be, but eh...
var bool                bUsingSword;
var float               fSwordFXTime;       // Starts at 0, counts up
var float               fSwordFXTimeSpan;   // = 3
var float               fSwordLength;
var float               fSwordFXStartScale; // Starts at 0, counts up
var float               fSwordFXEndScale;   // = 3
var particleFX			fxSwordParticles;   // Sword particles used when charging up for a cast	 

// --- Dueling specific
var bool				bGlowingWand;

// --- Debug
var bool				bUseDebugMode;
var Harry				playerHarry;

//************************
//** Spell list for HP2 **
//************************
//
// --- In game spells
// 1.) Flipendo		- pushes things
// 2.) Lumos		- lights up your wand
// 3.) Alohomora	- unlock doors, secret areas
// 4.) Skurge		- gets rid of slime
// 5.) Rictusempra	- same as punching someone in the gut
// 6.) Diffindo		- cuts things
// 7.) Spongify		- makes things bouncy
//
// --- Wizard Duel only spells 
// 8.) Rictusempra  - same as in game spell but takes away spell energy
// 9.) Mimblewimble	- causes opponent wizard to mumble words that don't make sense
//10.) Expelliarmus	- spell rebound



// --------------------------------------------------------------------------------------------
// *** Constants



function SetDebugMode( bool bOn )	{ bUseDebugMode = bOn; }

// --------------------------------------------------------------------------------------------
// *** Functions

simulated function PostBeginPlay()
{
	Super.PostBeginPlay();
	
	//Always do this, so the other spells can use playerHarry.
	playerHarry = Harry(Level.playerHarryActor);

	// Lumos was off when we tried to turn it on, so lets spawn a light
	TheLumosLight = spawn(class'LumosLight',[SpawnOwner]self, [SpawnLocation]location );
	
	if( TheLumosLight == None )
		playerHarry.ClientMessage("ERROR!!! LumosLight could not be spawned!!!!!");
}

function PreBeginPlay()
{	
	fxSwordParticles = spawn( class'SwordBlade2FX' );
	fxSwordParticles.EnableEmission( false );
}

event Destroyed()
{
	if( fxChargeParticles != none )
		fxChargeParticles.Destroy();

	if( fxSwordParticles != none )
		fxSwordParticles.Destroy();
	
	if( TheLumosLight != none )
		TheLumosLight.Destroy();

	super.Destroyed();
}

function float ChargingLevel()
{
	if( bUsingSword )
	{
		if( fxSwordParticles.bEmit )
		{
			//if( bReturnInverse )
			//	return 1 - fSwordFXTime / fSwordFXTimeSpan;
			//else
				return     fSwordFXTime / fSwordFXTimeSpan;
		}
		else
		{
			//if( bReturn1WhenNotCharging )
			//	return 1.0;
			//else
				return 0;
		}
	}
	else
	{
		return fSpellCharge;
	}
}

function StartGlowingWand( class<baseSpell> GlowSpellClass )
{
	if( fxChargeParticles != None )
		fxChargeParticles.Destroy();
	
	fxChargeParticles = spawn( GetChargeParticleClass( GlowSpellClass ) );
	fxChargeParticles.bEmit = true;
	bGlowingWand = true;
}

function StopGlowingWand()
{
	bGlowingWand = false;

	if( fxChargeParticles != None )
		fxChargeParticles.Destroy();
}

function StartChargingSpell( bool bChargeSpell, 
							optional bool in_bHarryUsingSword, 
							optional class<baseSpell> ChargeSpellClass )
{
	bSpellCharges = bChargeSpell;

	if( in_bHarryUsingSword )
		fxSwordParticles.EnableEmission( true );
	else
	{
		if( fxChargeParticles != None )
			fxChargeParticles.Destroy();
		
		// create our wand particleFX then make bEmit = false
		
		if( ChargeSpellClass != None )
		{
			// get the ChargeParticleClass depending upon the spell
			fxChargeParticles = spawn( GetChargeParticleClass( ChargeSpellClass) );
		}
		else
		{
			fxChargeParticles = spawn( default.fxChargeParticleFXClass );
			fxChargeParticles.SizeWidth.Base	= 8;
			fxChargeParticles.SizeLength.Base	= 8;
		}

		fxChargeParticles.EnableEmission( true );
	}

	fSpellChargeTime = 0;
	fSpellCharge	 = 0;
	fSwordFXTime     = 0;
}

function StopChargingSpell()
{
	bSpellCharges    = false;
	fSpellChargeTime = 0;
	fSpellCharge	 = 0;
		
	// If the wand is glowing then scale particles back to their original scale factor
	if( bGlowingWand )
		ScaleParticles( fxChargeParticles, 1.0f );
	else // the wand is not glowing so shut down our charge particles
		fxChargeParticles.Shutdown();

	fxSwordParticles.EnableEmission( false );
}

// Mainly used for wizard dueling because you charge your spells in wizard dueling
function class<ParticleFX> GetChargeParticleClass( class<baseSpell> spellClass )
{
	switch( spellClass )
	{
		case class'spellFlipendo'		 : return class'flip_fly';
		case class'spellLumos'			 : return class'lumos_fly';
		case class'spellAlohomora'		 : return class'aloh_fly';
		case class'spellSkurge'			 : return class'skurge_fly';
		case class'spellRictusempra'	 : return class'rictusempra_fly';
		case class'spellDiffindo'		 : return class'diffindo_fly';
		case class'spellSpongify'		 : return class'spongify_fly';
		
		// --- Wizard Duel only
		case class'spellDuelRictusempra' : return class'duelRictusempra_fly';
		case class'spellDuelMimblewimble': return class'duelMimblewimble_fly';
		case class'spellDuelExpelliarmus': return class'duelExpelliarmus_fly';
		
		default: break;
	}

	return class'flip_fly';
}

function class<baseSpell> GetClassFromSpellName( string SpellName )
{
	switch( SpellName )
	{
		case "Flipendo":			return class'spellFlipendo';
		case "Lumos":				return class'spelllumos';
		case "Alohomora":			return class'spellAlohomora';
		case "Skurge":				return class'spellSkurge';
		case "Rictusempra":			return class'spellRictusempra';
		case "Diffindo":			return class'spellDiffindo';
		case "Spongify":			return class'spellSpongify';
		
		case "DuelRictusempra":		return class'spellDuelRictusempra';
		case "DuelMimblewimble":	return class'spellDuelMimblewimble';
		case "DuelExpelliarmus":	return class'spellDuelExpelliarmus';
	}
	return None;
}


function class<baseSpell> GetClassFromSpellType( ESpellType eSpellType )
{
	switch( eSpellType )
	{
		case SPELL_Flipendo:		return class'spellFlipendo';
		case SPELL_Lumos:			return class'spelllumos';
		case SPELL_Alohomora:		return class'spellAlohomora';
		case SPELL_Skurge:			return class'spellSkurge';
		case SPELL_Rictusempra:		return class'spellRictusempra';
		case SPELL_Diffindo:		return class'spellDiffindo';
		case SPELL_Spongify:		return class'spellSpongify';

		case SPELL_DuelRictusempra:	return class'spellDuelRictusempra';
		case SPELL_DuelMimblewimble:return class'spellDuelMimblewimble';
		case SPELL_DuelExpelliarmus:return class'spellDuelExpelliarmus';
	}
	return None;
}

function SetCurrentSpell( class<baseSpell> spellClass, optional bool bForceSelection )
{
	if( owner.IsA('Harry') )
	{
		// If we can use this spell then set it as our current spell
		if( Harry(owner).IsInSpellBook( spellClass.default.spellType )  ||  bForceSelection )
		{
			CurrentSpell = spellClass;
		}
		else
		{
			if( bUseDebugMode ) 
				playerHarry.ClientMessage( "HARRY CAN NOT USE THIS SPELL YET!!!! -> " $spellClass );
		}
	}
	else
	{
		// if we have an owner that is NOT harry then just set the current spell
		CurrentSpell = spellClass;
	}
}

function ChooseSpell( ESpellType eSpellType, optional bool bForceSelection )
{
	SetCurrentSpell( GetClassFromSpellType( eSpellType ), bForceSelection );
}

// we need to find chargeParticleFXScale outside of our baseWand (in baseSpell ).
function float GetChargeParticleFXScale( float fCharge )
{
	if( fCharge > 1.0f )
		return fSpellChargeEndScale + fCharge;
	else
		return fSpellChargeStartScale + (( fSpellChargeEndScale - fSpellChargeStartScale ) * fCharge );
}

function SetInstantFire( bool in_bInstantFire )
{
	bInstantFire = in_bInstantFire;
}

function ToggleUseSword()
{
	bUsingSword = !bUsingSword;

	fxChargeParticles.EnableEmission( false );
	fxSwordParticles.EnableEmission( false );

	if( bUsingSword )
		Mesh = Mesh'skGryf_SwordMesh';
	else
		Mesh = Mesh'wandmesh';

	ThirdPersonMesh = Mesh;
}

function vector GetWandEndPoint()
{
	return	pawn(owner).weaponLoc - (vec(0,0,20) >> pawn(owner).weaponRot);
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

event Tick( float fTimeDelta )
{
	local vector WandEndPoint;
	local float  scale;

	super.Tick( fTimeDelta );
	

	// *** Get the wand's endPoint
	if( pawn(owner) != None && pawn(owner).Weapon == self )
	{
		if( bUsingSword )
		{
			//Only move up the sword if the particles are emitting.
			if( fxSwordParticles.bEmit )
				fSwordFXTime += fTimeDelta;

			fSwordFXTime = FMin(fSwordFXTime, fSwordFXTimeSpan);

 			if( fSwordFXTime >= 1.9  &&  fSwordFXTime - fTimeDelta < 1.9 )
				PlaySound(sound'HPSounds.Magic_sfx.sword_loop', SLOT_Interact);

			scale = fSwordFXTime / fSwordFXTimeSpan;
			//fxSwordParticles.DrawScale = fSwordFXStartScale + (fSwordFXEndScale - fSwordFXStartScale) * scale;

			WandEndPoint = pawn(owner).weaponLoc - (vec(0,0,fSwordLength*scale) >> pawn(owner).weaponRot);
			fxSwordParticles.SetLocation( WandEndPoint );
			
			ScaleParticles(fxSwordParticles, fSwordFXStartScale + (fSwordFXEndScale - fSwordFXStartScale) * scale );
		}
		else if( fxChargeParticles.bEmit || TheLumosLight.bLumosOn )
		{
			if( bSpellCharges && fSpellCharge < 1.0 )
			{
				fSpellChargeTime = FMin(fSpellChargeTime + fTimeDelta, fSpellChargeTimeSpan);
				fSpellCharge	 = FMin( 1.0f, fSpellChargeTime / fSpellChargeTimeSpan );
								
				ScaleParticles( fxChargeParticles, GetChargeParticleFXScale( fSpellCharge ) );
			}

			WandEndPoint = GetWandEndPoint();

			// *** Update wand particles
			if( fxChargeParticles != None )
				fxChargeParticles.SetLocation( WandEndPoint );
		}
		
	}
	
	// *** Update Lumos
	if( TheLumosLight.bLumosOn )
		TheLumosLight.UpdateLocation( WandEndPoint );
}

function ShowCastedSpellList( optional int iNumSpells )
{
	local int  i;
	if( iNumSpells == 0 || iNumSpells > MAX_NUM_CASTED_SPELLS )
		iNumSpells = MAX_NUM_CASTED_SPELLS;
	
	playerHarry.ClientMessage("***Number of Casted Spells: " $NumCastedSpells);
	for( i=0; i<iNumSpells; ++i )
		playerHarry.ClientMessage(" spell[" $i $"] = " $CastedSpellList[i] );
}

function AddToCastedSpellList( baseSpell spell )
{
	if( spell == None )
		return;

	CastedSpellList[NumCastedSpells] = spell;
	LastCastedSpell = spell;
	NumCastedSpells++;
	
	if( NumCastedSpells > MAX_NUM_CASTED_SPELLS-1 )
	{
		NumCastedSpells = MAX_NUM_CASTED_SPELLS-1;
		playerHarry.clientmessage("!!!MAX NUMBER OF CASTED SPELLS REACHED for " $self.owner $" !!!!!!");
	}
}

function SubtractFromCastedSpellList( baseSpell spell )
{
	local int  i, index;
	local bool bFound;

	// find the spell in our list of casted spells
	for(i=0; i<NumCastedSpells; ++i)
	{
		if( CastedSpellList[i] == spell )
		{
			bFound = true;
			index = i;
			CastedSpellList[i] = None;
			NumCastedSpells--;
			break;
		}
	}
	
	if( bFound )
	{		
		// re orginize our array so that the gap is gone
		for(i=index; i<MAX_NUM_CASTED_SPELLS-1; ++i)
			CastedSpellList[i] = CastedSpellList[i+1];
	}
	else
	{
		playerHarry.ClientMessage("baseWand: Could not find spell: " $spell $" to subtract from list!!!" );
	}
}


function FlashChargeParticles( class<ParticleFX> classFX )
{
	// Destroy our charge particles (if we have any) then spawn expelliarmus effects on the wand
	fxChargeParticles.Destroy();
	fxChargeParticles = spawn( classFX );
}

//*************************************************************************************************************
//"New" functionality.  If spellType is !none, use that as the spell to cast.  Also, if spellType is !none,
// cast that spell regardless of whether aTarget is none or not.
// Also, if aTarget is self, then cast the spell in the actual direction of the weapon.
function CastSpell( optional actor			  aTarget, 
				    optional vector			  aTargetOffset, 
				    optional class<baseSpell> spellClass )
{
	local bool		bUseWeaponForProjRot;
	
	if( aTarget == self )
	{
		bUseWeaponForProjRot = true;
		aTarget = none;
	}

	if( spellClass != none )
	{
		// Since spellClass was passed explicitly, we dont have to check to see if we "have" this spell.
		CurrentSpell = spellClass;
	}
	else // AutoSelect the spell (if enabled)
	if( bAutoSelectSpell  &&  aTarget != None )	
		ChooseSpell( aTarget.eVulnerableToSpell );
	
	//*** TESTING SPELLS ***
	//	ChooseSpell( SPELL_Alohomora );
	//	ChooseSpell( SPELL_Skurge );
	//	ChooseSpell( SPELL_Rictusempra );
	//**********************

	// DEBUG
	if( bUseDebugMode) playerHarry.ClientMessage( "Casting spell " $CurrentSpell $" at " $aTarget );

	// Check to see if we have a valid spell selected
	if( CurrentSpell == None )
		return;

	//Create the spell if we're not using the sword,
	// or, if we are using the sword, and we're charged up enough.  Damn, I wish I'd made the sword a seperate weapon.  oops.
	if(   !bUsingSword
	   ||  bUsingSword && SwordChargedUpEnough()
	  )
	{
		// Create the spell and save a refrence to it
		AddToCastedSpellList( baseSpell( ProjectileFire(CurrentSpell, AltProjectileSpeed, false, bUseWeaponForProjRot)) );
		LastCastedSpell.bUseDebugMode = bUseDebugMode;
		LastCastedSpell.InitSpell( Owner, aTarget, aTargetOffset, fSpellCharge, self );
			
		// This sucks.  If sword spell, pass along how long it was charged up
		if( spellSwordFire(LastCastedSpell) != none )
			spellSwordFire(LastCastedSpell).DamagePercent( fSwordFXTime / fSwordFXTimeSpan );
		
		// Say the magic word
		LastCastedSpell.PlayIncantationSound( Owner );
	}
	
	// stop charging the spell we just casted
	StopChargingSpell();

	// Check to see if we are really close to our target, if so then autoHit our target!
	if( aTarget.IsA('HPawn') && 
		vsize(location - aTarget.location) < fAutoHitDistance )
	{
		// We are really close to our target so AutoHit it!
		LastCastedSpell.ProcessTouch( aTarget, aTarget.location );
		playerHarry.cm("Spell AutoHit Target " $aTarget $" because TargetDist " $vsize(location - aTarget.location) $" < " $fAutoHitDistance );
	}
}

function bool SwordChargedUpEnough()
{
	return fSwordFXTime >= fSwordFXTimeSpan/2;
}

function AltFire( float Value )
{
	if( playerHarry != None )
	{
		playerHarry.ClientInstantFlash( -0.4, vect(0, 0, 800));
		playerHarry.ShakeView(ShakeTime, ShakeMag, ShakeVert);
	}
	
	// Create our spell projectile!
	ProjectileFire( AltProjectileClass, AltProjectileSpeed, bAltWarnTarget, false );
	
//	Owner.PlaySound(AltFireSound, SLOT_None, Pawn(Owner).SoundDampening*4.0);
	PlayAnim('all', 0.8, 0.05);
	
	if( Owner.bHidden )
		CheckVisibility();
}

// creates the spell as a projectile
function Projectile ProjectileFire(class<projectile> ProjClass, float ProjSpeed, bool bWarn, optional bool bUseWeaponForProjRot)
{
	local vector	 vStart;
	local vector	 vEnd;
	local float		 fDistance;
	local rotator	 r;
	local Projectile proj;

	// Make projectileFire sound
	Owner.MakeNoise( Pawn(Owner).SoundDampening );
	
	// The starting position of the spell is at the tip of the wand.
	if( bUsingSword )
	{
		vStart = pawn(owner).weaponLoc - (vec(0,0,fSwordLength * fSwordFXTime/fSwordFXTimeSpan) >> pawn(owner).weaponRot);
		//vEnd = harry(owner).GetSwordFireTargetLoc();
		vEnd = harry(owner).SpellCursor.Location;
		if( bUseWeaponForProjRot )
		{
			r = pawn(owner).weaponRot;
		}
		else
		{
			if( vEnd == vect(0,0,0) )
				r = harry(owner).cam.rotation;
			else
				r = rotator(vEnd - vStart);
		}

		proj = spawn( ProjClass,owner,, [SpawnLocation]vStart, [SpawnRotation]r );
	}
	else
	{
		vStart = pawn(owner).weaponLoc + (vec(0,0,20) >> pawn(owner).weaponRot);
		
		if( bUseWeaponForProjRot )
		{
			r = pawn(owner).weaponRot;
		}
		else
		{
			if( owner.IsA('Harry') )
				r = harry(owner).cam.rotation;
			else
				r = pawn(owner).rotation;
		}

		proj = spawn( ProjClass,owner,, [SpawnLocation]vStart, [SpawnRotation]r );
		
		if( proj == None )
		{
			// Most likely the vStart location is invalid, lets try to have the start location in a diffrent place
			if( pawn(owner).IsA('PlayerPawn') )
				vStart = PlayerPawn(owner).Location + vec(0,0,PlayerPawn(owner).EyeHeight);
			else
			if( pawn(owner).IsA('Pawn') )
				vStart = pawn(owner).Location;
			
			proj = spawn( ProjClass, owner, , [SpawnLocation]vStart, [SpawnRotation]r );
		}
	}

	// Spawn our spell at the Start location with the AdjustedAim rotation
	return proj;
}


//***************************************************************************************
//** LUMOS Specific Functions
//***************************************************************************************

function bool IsLumosOn() 
{ 
	return playerHarry.bLumosOn; 
}

function LumosTurnOn()
{
	TheLumosLight.TurnOn();
}

//***************************************************************************************
//** UNREAL WEAPON Specific Functions ( from HP1 )
//***************************************************************************************

function BecomeItem()
{
	//Inventory::BecomeItem sets bHidden to true.
	Super.BecomeItem();
	
	bHidden = false;
}

function Texture GetSpellIcon()
{
	if( CurrentSpell!=None )
		return CurrentSpell.Default.spellIcon;
	else
		return none;
}

function inventory SpawnCopy( pawn Other )
{
	local inventory Copy;
	local Inventory I;

	Copy = Super.SpawnCopy( Other );

	return Copy;
}


function float RateSelf( out int bUseAltMode )
{
	return 99.0;	//wand is always the best weapon. well, its the _only_ weapon. 
}

function BecomePickup()
{
	Super.BecomePickup();
}

function Finish()
{
	if ( (Pawn(Owner).bFire!=0) && (FRand() < 0.6) )
		Timer();
	Super.Finish();
}


function PlayFiring()
{
	//Owner.PlaySound(FireSound, SLOT_None, Pawn(Owner).SoundDampening*4.0);
	PlayAnim('all', 0.5,0.05);
}

function PlayIdleAnim()
{

}


// --------------------------------------------------------------------------------------------
// *** States



// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- base Wand
	bUseDebugMode=false
		
	bAutoSelectSpell=true
	
	// --- HWeapon
	ItemName="Wand"
	bSplashDamage=true
	AltProjectileClass=class'baseSpell'
	AmmoName=Class'castAAmmo'
	PickupAmmoCount=200
	bInstantHit=false
	bAltWarnTarget=false
	FireOffset=(X=0.000000,Y=-6.000000,Z=-7.000000)
	AltRefireRate=0.700000
//	FireSound=Sound'HPSounds.genericSpellCastSound'
//	AltFireSound=Sound'HPSounds.genericSpellCastSound'
	
	AutoSwitchPriority=4
	InventoryGroup=4
	PickupMessage="You got the ASMD"
	PlayerViewOffset=(X=3.500000,Y=-1.800000,Z=-2.000000)
	
	ThirdPersonMesh=Mesh'wandmesh'
//	PickupSound=Sound'UnrealShare.Pickups.WeaponPickup'
	bNoSmooth=False
	bMeshCurvy=False
	CollisionRadius=28.000000
	CollisionHeight=8.000000
	Mass=50.000000
	DeathMessage="%k inflicted mortal damage upon %o with the %w."
	
	aimerror=0.0
	
	Mesh=Mesh'WandMesh'
	DrawType=DT_Mesh
	bStatic=False
	
	// Spell Charge
	fxChargeParticleFXClass=class'skurge_fly'
	fSpellChargeTimeSpan=1.0f						//3
	fSpellChargeStartScale=1.0f
	fSpellChargeEndScale=3.0f
	
	fSwordFXTimeSpan=2
	fSwordFXStartScale=0.1; // Starts at 0, counts up
	fSwordFXEndScale=6;   // = 3
	fSwordLength=55
	
	fAutoHitDistance=128
}




// --------------------------------------------------------------------------------------------
// baseWand.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------
