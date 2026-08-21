// --------------------------------------------------------------------------------------------
//  _                                 _____                  _    _                         
// | |                               / ____|                | |  | |                        
// | |     _   _ _ __ ___   ___  ___| (___  _ __   __ _ _ __| | __ | ___ ___     _   _  ___ 
// | |    | | | | '_ ` _ \ / _ \/ __|\___ \| '_ \ / _` | '__| |/ / |/ _ \ __|   | | | |/ __|
// | |____| |_| | | | | | | (_) \__ \____) | |_) | (_| | |  |   <| |  __/__ \ _ | |_| | (__ 
// |______|\__,_|_| |_| |_|\___/|___/_____/| .__/ \__,_|_|  |_|\_\_|\___|___/(_) \__,_|\___|
//                                         | |                                              
//                                         |_|                                              
// --------------------------------------------------------------------------------------------
// Class Name  : LumosSparkles
//
// Created on  : 06/12/2002
// 
// Description : LumosSparkles will emit Sparkles when the lumos spell is near
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class LumosSparkles extends Triggers;

// --------------------------------------------------------------------------------------------
// *** Variables

var Harry					playerHarry;	// refrence to harry

var float					fArea;			// h*w*d = area

var()float					fAreaSparklesPerSquareGameUnit;
var()ParticleFX				fxAreaSparkles;		
var()class<ParticleFX>		fxAreaSparklesClass;


var()float					fEdgeSparklesThickness;
var()ParticleFX				fxEdgeSparkles[12];		
var()class<ParticleFX>		fxEdgeSparklesClass;

var()float					fDistanceForAreaEffects;
var()float					fDistanceForEdgeEffects;

var float					fDistanceToLumosSource;

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PostBeginPlay()
{
	Super.PostBeginPlay();
	playerHarry = Harry(Level.playerHarryActor);
}

event Destroyed()
{
	TurnOffAreaEffects();
	TurnOffEdgeEffects();
}

function TurnOnAreaEffects()
{
	local vector hwd, hwdRotated;
	
	// start visualFX
	if( fxAreaSparkles == None )
	{
		// rotate our collision d,w,h so that it corisponds with rotated Areas
		if( CollideType == CT_AlignedCylinder || CollideType == CT_OrientedCylinder || CollisionWidth == 0)
			hwd = vec(CollisionRadius, CollisionRadius, CollisionHeight );
		else
			hwd = vec(CollisionRadius, CollisionWidth, CollisionHeight  );
		
		hwdRotated = hwd >> rotation;
		
		fxAreaSparkles = spawn( fxAreaSparklesClass,[SpawnOwner]self, [SpawnLocation]location );
		
		// Create sparkels that have a width and height == to the collision bbox	
		fxAreaSparkles.SourceDepth.Base		= hwdRotated.x*2; // depth
		fxAreaSparkles.SourceWidth.Base		= hwdRotated.y*2; // width
		fxAreaSparkles.SourceHeight.Base	= hwdRotated.z*2; // height
				
		fArea = vsize(hwd);
	}

}


function TurnOffAreaEffects()
{
	// stop visualFX
	if( fxAreaSparkles != None )
	{
		fxAreaSparkles.ShutDown();
	}
}


function TurnOnEdgeEffects()
{
	local BoundingBox	bbox;
	local float	fBoxWidth, fBoxHeight, fBoxDepth;
	local int	i;
	
	// start visualFX
	if( fxEdgeSparkles[0] == None )
	{
		bbox = GetWorldCollisionBox( true );
		
		// Calculate the width, depth and height of our bbox
		fBoxWidth  = bbox.Max.x - bbox.Min.x;
		fBoxDepth  = bbox.Max.y - bbox.Min.y;
		fBoxHeight = bbox.Max.z - bbox.Min.z;
		
		for(i=0;i<12;++i)
		{
			fxEdgeSparkles[i] = spawn( fxEdgeSparklesClass,[SpawnOwner]self, [SpawnLocation]location );
			fxEdgeSparkles[i].SourceDepth.Base	= fEdgeSparklesThickness;
			fxEdgeSparkles[i].SourceWidth.Base	= fEdgeSparklesThickness;
			fxEdgeSparkles[i].SourceHeight.Base	= fEdgeSparklesThickness;
		}
		
		// Right Top
		fxEdgeSparkles[0].SetLocation( location + vec((fBoxWidth / 2), 0, (fBoxHeight / 2)) );
		fxEdgeSparkles[0].SourceWidth.Base	= fBoxDepth;		
		
		// Right Bottom
		fxEdgeSparkles[1].SetLocation( location + vec((fBoxWidth / 2), 0, -(fBoxHeight / 2)) );
		fxEdgeSparkles[1].SourceWidth.Base	= fBoxDepth;		
		
		// Left Top
		fxEdgeSparkles[2].SetLocation( location + vec(-(fBoxWidth / 2), 0, (fBoxHeight / 2)) );
		fxEdgeSparkles[2].SourceWidth.Base	= fBoxDepth;				
		
		// Left Bottom
		fxEdgeSparkles[3].SetLocation( location + vec(-(fBoxWidth / 2), 0, -(fBoxHeight / 2)) );
		fxEdgeSparkles[3].SourceWidth.Base	= fBoxDepth;				
		
		// Front Top
		fxEdgeSparkles[4].SetLocation( location + vec(0, (fBoxDepth / 2), (fBoxHeight / 2)) );
		fxEdgeSparkles[4].SourceDepth.Base	= fBoxWidth;		
		
		// Front Bottom
		fxEdgeSparkles[5].SetLocation( location + vec(0, (fBoxDepth / 2), -(fBoxHeight / 2)) );
		fxEdgeSparkles[5].SourceDepth.Base	= fBoxWidth;
		
		// Back Top
		fxEdgeSparkles[6].SetLocation( location + vec(0,-(fBoxDepth / 2), (fBoxHeight / 2)) );
		fxEdgeSparkles[6].SourceDepth.Base	= fBoxWidth;		
		
		// Back Bottom
		fxEdgeSparkles[7].SetLocation( location + vec(0,-(fBoxDepth / 2), -(fBoxHeight / 2)) );
		fxEdgeSparkles[7].SourceDepth.Base	= fBoxWidth;
		
		// Up/Down bars
		fxEdgeSparkles[8].SetLocation( location + vec((fBoxWidth / 2), (fBoxDepth / 2), 0) );
		fxEdgeSparkles[8].SourceHeight.Base	= fBoxHeight;
		
		fxEdgeSparkles[9].SetLocation( location + vec(-(fBoxWidth / 2), (fBoxDepth / 2), 0) );
		fxEdgeSparkles[9].SourceHeight.Base	= fBoxHeight;		
		
		fxEdgeSparkles[10].SetLocation( location + vec((fBoxWidth / 2), -(fBoxDepth / 2), 0) );
		fxEdgeSparkles[10].SourceHeight.Base= fBoxHeight;
		
		fxEdgeSparkles[11].SetLocation( location + vec(-(fBoxWidth / 2), -(fBoxDepth / 2), 0) );
		fxEdgeSparkles[11].SourceHeight.Base= fBoxHeight;		
	}

	// start soundFX
	PlaySound( 	Sound'HPSounds.Magic_sfx.Lumos_glow_loop', 
				SLOT_Interact,	// slot
				0.5,		// volume
				true,		// bNoOverride
				,			// radius
				,			// pitch
				,			// disable3D
				true );		// loop
}


function TurnOffEdgeEffects()
{
	local int i;

	// stop visualFX
	if( fxEdgeSparkles[0] != None )
	{
		for(i=0;i<12;++i)
		{
			if( fxEdgeSparkles[i] != None )
			{
				fxEdgeSparkles[i].ShutDown();
				fxEdgeSparkles[i] = None;
			}
		}
	}
	
	// stop soundFX
	StopSound( Sound'HPSounds.Magic_sfx.Lumos_glow_loop', SLOT_Interact );	

}


function UpdateSparkles()
{
	// Update the Distance To Lumos Source
	fDistanceToLumosSource = vsize(playerHarry.cam.location - location);
	
	if( fDistanceToLumosSource < fDistanceForEdgeEffects )
	{
		// Turn On Edge Sparkels
		TurnOnEdgeEffects();
	}
	else
	{
		// Turn off Edge Sparkels
		TurnOffEdgeEffects();
	}
	
	// if we are within the lumos radius then turn on!	
	if( fDistanceToLumosSource < fDistanceForAreaEffects )
	{
		// Turn On Area Sparkles (if not on already)
		TurnOnAreaEffects();
		
		fxAreaSparkles.ParticlesPerSec.Base	= fArea * fAreaSparklesPerSquareGameUnit;
		fxAreaSparkles.ParticlesPerSec.Base	-= (fDistanceToLumosSource * 0.015);
		
		if( fxAreaSparkles.ParticlesPerSec.Base < 0.025f )
			fxAreaSparkles.ParticlesPerSec.Base = 0.025f;
	}
	else
	{
		// Turn off Area Sparkles
		TurnOffAreaEffects();
	}
}


// --------------------------------------------------------------------------------------------
// *** States

auto state StateLumosOff
{
	function beginState()
	{
		playerHarry.clientmessage(" LumosSparkles " $self $" BeginState LumosOff called!!" );
	}

	// When harry turns on/off lumos then this function will be called
	function OnLumosOn()
	{
		GotoState('StateWaitingToTurnOn');
	}
}

state StateWaitingToTurnOn
{	
	event Tick( float fTimeDelta )
	{
		// Update the Distance To Lumos Source
		fDistanceToLumosSource = vsize(playerHarry.cam.location - location);

		// if we are within the lumos radius then turn on!	
		if( fDistanceToLumosSource < fDistanceForAreaEffects )
			GotoState('StateLumosOn');
	}
	
	function OnLumosOff()	
	{ 	
		GotoState('StateLumosOff');
	}
}

state StateLumosOn
{
	event Tick( float fTimeDelta )
	{	
		// Update the sparkles
		UpdateSparkles();
	}

	function OnLumosOff()	
	{ 
		GotoState('StateLumosOff');
	}
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	bHidden=true
	
	fEdgeSparklesThickness=4.0f
	
	fAreaSparklesPerSquareGameUnit=0.25f
	fDistanceForAreaEffects=1536.0f
	fDistanceForEdgeEffects=768.0f
	
	fxAreaSparklesClass=class'Lumos_react'
	fxEdgeSparklesClass=class'diffindo_RopeFx'
	
	CollideType=CT_Box
	CollisionRadius=+00128.000000
	CollisionHeight=+00128.000000
}


// --------------------------------------------------------------------------------------------
// LumosSparkles.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------

