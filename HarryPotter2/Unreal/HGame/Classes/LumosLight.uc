// --------------------------------------------------------------------------------------------
//  _                                _      _       _     _                  
// | |                              | |    (_)     | |   | |                 
// | |     _   _ _ __ ___   ___  ___| |     _  __ _| |__ | |_     _   _  ___ 
// | |    | | | | '_ ` _ \ / _ \/ __| |    | |/ _` | '_ \| __|   | | | |/ __|
// | |____| |_| | | | | | | (_) \__ \ |____| | (_| | | | | |_  _ | |_| | (__ 
// |______|\__,_|_| |_| |_|\___/|___/______|_|\__, |_| |_|\__|(_) \__,_|\___|
//                                             __/ |                         
//                                            |___/                          
// --------------------------------------------------------------------------------------------
// Class Name  : LumosLight
//
// Created on  : 06/11/2002
// 
// Description : A Lumos Light is used to manage lumos during its lifetime. It will announce
//				 to everyone when lumos turns on and off.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class LumosLight extends Actor;

// --------------------------------------------------------------------------------------------
// *** Variables
var bool				bLumosOn;
var particleFX			Particles;			// lumos particleFX used when lumos is active

var float				fLumosTimeToTurnOff;// lumos time to turn off
var float				fLumosTime;			// LumosTimeLeft

var bool				bInfiniteLumos;		// Are we in infinite Lumos mode?

// --- Debug
var bool				bUseDebugMode;
var Harry				playerHarry;


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions

// override all projectile functions
function bool EncroachingOn( actor Other )							{ return false; }
simulated singular function Touch(Actor Other)						{ }
simulated function ProcessTouch(Actor Other, Vector HitLocation)	{ }
simulated function HitWall (vector HitNormal, actor Wall)			{ }
simulated function Explode(vector HitLocation, vector HitNormal)	{ }
function bool IsRelevantToMover()									{ return false; }

function PreBeginPlay()
{
	//Always do this, so the other spells can use playerHarry.
	playerHarry = Harry(Level.playerHarryActor);

	SetPhysics(PHYS_None);
	// SetCollision( collide actors, block actors, block players )
	SetCollision( false, false, false );
	
	bLumosOn   = false;
	Disable('Tick');
	playerHarry.bLumosOn = false;
}

event Destroyed()
{
	// DEBUG
	playerHarry.ClientMessage("LumosLight: Destroyed() CALLED!!!!!!!!!!!!!! while playerHarry.bLumosOn = " $playerHarry.bLumosOn );
	
	TurnOff();	
	if( Particles != None )
		Particles.Destroy();
}

event FellOutOfWorld()
{
	// over ride the actors version so we don't destroy ourselves
}


function ShowDebugInfo()
{
	playerHarry.ClientMessage("bLumosOn = " $bLumosOn );

	playerHarry.ClientMessage("LightRadius   = " $LightRadius );
	playerHarry.ClientMessage("fLumosTime = " $fLumosTime );
	
	playerHarry.ClientMessage("player.bLumosOn = " $playerHarry.bLumosOn );
}

function ScaleParticles( float fScale )
{
	// error checking
	fScale = fclamp(fScale, 0.0f, 1.0f);
	
	// Scale our light radius
	LightRadius = 5 + (10 * fScale);
	
	// Scale our particleFX
	Particles.SizeWidth.Base  = Particles.default.SizeWidth.Base  * fScale;
	Particles.SizeLength.Base = Particles.default.SizeLength.Base * fScale;
}

function Tick( float fTimeDelta )
{
	if( !bLumosOn )
		return;
	
	// Subtract lumos time
	fLumosTime += fTimeDelta;	
	
	if( bInfiniteLumos )
	{
		// Lumos is infinite so scale our particles smoothly up and down, over time
		ScaleParticles( 0.75f - (0.5f*Abs(sin(fLumosTime*0.25f))) );
		return;
	}
	else if( fLumosTime > fLumosTimeToTurnOff )
	{
		fLumosTime = fLumosTimeToTurnOff;
		TurnOff();
	}
	
	// As lumosTimeLeft goes away scale the Particles on the tip of the wand
	ScaleParticles( 1.0f - (fLumosTime / fLumosTimeToTurnOff) );
}

function UpdateLocation( vector NewLocation )
{		
	SetLocation( NewLocation );
	Particles.SetLocation( NewLocation );
}

function TurnOn()
{
	local actor A;

	if( bUseDebugMode ) playerHarry.ClientMessage("LumosLight.TurnOn()  Resetting Lumos counter to 0, was " $fLumosTime );
	
	// Reset our LumosTime
	fLumosTime	 = 0.0f;

	if( playerHarry.bLumosOn )
		return;
	
	// we need a boolean in LumosLight because playerHarry is not always valid
	// reset our lumos time
	bLumosOn	 = true;
	playerHarry.bLumosOn = true;
	Enable('Tick');

	TurnDynamicLightOn();
	

	// Announce that lumos is on
	foreach AllActors( class'actor', A )
		(A).OnLumosOn();
	
	// Create our particleFX right allong with our light
	if(Particles != None ) Particles.Destroy();
	
	// Create our particleFX right allong with our light
	Particles = spawn( class'LumosLightFX', [SpawnOwner]self, [SpawnLocation]location );
	
	if( Particles == None )
		playerHarry.ClientMessage("ERROR!!! Particles could not be spawned!!!!!");
	
	Particles.EnableEmission( true );	
	
	// DEBUG
	if( bUseDebugMode )
		playerHarry.ClientMessage("LumosLight: Successfuly turned lumos ON!!! while bLumosOn = " $playerHarry.bLumosOn
								  $" fLumosTime = " $fLumosTime );
}

function TurnOff()
{
	local actor A;
	
	if( bUseDebugMode )
		playerHarry.ClientMessage("LumosLight: TurnOff() called! " );

	// We need to store the bLumosOn boolean in "playerPawn" so that the engine can get access to it.
	fLumosTime = 30.0f;
	bLumosOn   = false;
	playerHarry.bLumosOn = false;
	Disable('Tick');

	TurnDynamicLightOff();

	// Its safe to turn off lumos, so let everyone know we are turning it off.
	foreach AllActors( class'actor', A )
		(A).OnLumosOff();
	
	// Destroy our Particles
	if(Particles != None ) 
		Particles.Destroy();
	
	if( bUseDebugMode )
		playerHarry.ClientMessage("LumosLight: Successfuly turned lumos OFF!!! while bLumosOn = " $playerHarry.bLumosOn
							 	 $" fLumosTime = " $fLumosTime );
}

function TurnDynamicLightOn()
{
	// *** Turn on dynamic light
	
	// Dynamic Light Modulation
	LightType=LT_Steady;
	
	// Dynamic Light Spatial effect to use
	LightEffect=LE_NonIncidence;
	
	// Dynamic Light Color
	LightBrightness=400;
	LightHue=32;
	LightSaturation=72;
	
	// Dynamic Light Properties
	LightRadius=15;
	LightRadiusInner=5;
}

function TurnDynamicLightOff()
{
	// *** Turn on dynamic light
	
	// Dynamic Light Modulation
	LightType=LT_None;
	
	// Dynamic Light Spatial effect to use
	LightEffect=LE_None;
	
	// Dynamic Light Color
	LightBrightness=0;
	LightHue=0;
	LightSaturation=0;
	
	// Dynamic Light Properties
	LightRadius=0;
	LightRadiusInner=0;
}

// --------------------------------------------------------------------------------------------
// *** States


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	bUseDebugMode=true
	fLumosTime=0.0f
	fLumosTimeToTurnOff=30.0f
	bInfiniteLumos=false

	// --- Projectile
    bDirectional=false
    DrawType=DT_Mesh
    Texture=None
	RemoteRole=ROLE_SimulatedProxy
		
	bStatic=false
	style=sty_translucent  
	
	// Physics
	Physics=PHYS_None

	// Collision
	bCollideActors=false
	bCollideWorld=false
	bBlockActors=false
	bBlockPlayers=false

}

// --------------------------------------------------------------------------------------------
// LumosLight.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------

