// --------------------------------------------------------------------------------------------
//  _    _ _____  _  __  __ _           _                      
// | |  | |  __ \(_)/ _|/ _(_)         | |                     
// | |__| | |  | |_| |_| |_ _ _ __   __| | ___      _   _  ___ 
// |  __  | |  | | |  _|  _| | '_ \ / _` |/ _ \    | | | |/ __|
// | |  | | |__| | | | | | | | | | | (_| | (_) | _ | |_| | (__ 
// |_|  |_|_____/|_|_| |_| |_|_| |_|\__,_|\___/ (_) \__,_|\___|
//                                                             
//                                                             
// --------------------------------------------------------------------------------------------
// Class Name  : HDiffindo
//
// Created on  : 06/03/2002
// Authored by : Elijah Emerson
// 
// Description : A Diffindo Object is vunerable to diffindo and when hit by diffindo it will send an event then destroy itself.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class HDiffindo extends HProp;


// --------------------------------------------------------------------------------------------
// *** Variables

var ParticleFX			fxCut;					// ParticleFX for the cutting the diffindo obj
var class<ParticleFX>	fxCutClass;				// class that the cutFX is.
												
var ParticleFX			fxExplode;				// ParticleFX for when a Diffindo obj has exploded
												
var() class<ParticleFX>	fxExplodeClass0;		// class that the explodeFX is.
var() class<ParticleFX>	fxExplodeClass1;		// class that the explodeFX is.
var() class<ParticleFX>	fxExplodeClass2;		// class that the explodeFX is.
var() class<ParticleFX>	fxExplodeClass3;		// class that the explodeFX is.
												
												
var vector				vStartPoint;			// When making a cut this is the starting point
var vector				vEndPoint;				// This is the ending point of the cut
var	vector				vCutLength;				// vector describing the length of the cut
												
var BoundingBox			bbArea;					// Diffindo BBox Area
												
												
var   float				fCutTime;				// the cut timer will shorten over time (so that cuts happen faster over time)

var() float				fSingleCutTimer;		// time it takes to finish a single cut
var() float				fDiffindoTimer;			// How long does this object have diffindo hitFX?

var() bool				bUseDiffindoSpellHitFX;	// should this diffindo object explode using the diffindo explodeFX.

var() Sound				DiffindoImpactSound;	// Sound on impact
var() Sound				DiffindoCutSound;		// Sound after a cut

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PreBeginPlay()
{
	// Find harry and save him
	playerHarry = Harry(Level.playerHarryActor);
	
	// SetCollision( collide actors, block actors, block players )
	SetCollision( true, true, true );

	// Init our diffindo area bounding box
	bbArea = GetWorldCollisionBox( true );
	
	ComputeNewStartAndEndPoints();
}

event Destroyed()
{	
	// make sure our fx is shutdown
	if( fxCut != None )
		fxCut.Shutdown();

	Super.Destroyed();
}





function ComputeNewStartAndEndPoints()
{
	// We need to choose a start point and an end point for the diffindo cut FX.
	// To acomplish this task I am going to choose one of the 6 sides of the cube to have my start point
	// Then the end point will always be on the opposite side.
	//
	// (X) = max point
	// (O) = min point
	//  x  = point that connects to max point
	//  o  = point that connects to min point
	// 
	//
	//		( min sides -> left, back, bottom )
	//
	//            o--------  
	//           /|        |         
	//          / |  back  |       ^ Z  
	//            |        |       |    
	//         |  |        |        -->   
	// left -> | (O)-------o      /   X
	//         | /        /       Y
	//         |/ bottom /
	//         o-------- 
	//
	// -------------------------------------
	//
	//		( max sides -> right, front, top )
	//
	//       --------x           	
	//     /  top   /|   	
	//    /        / |<- right side                       
	//   x-------(X) |        
	//   |        |  |       ^ Z                       
	//	 |  front |          |                         
	//   |        | /         -->                        
	//   |        |/        /   X                    
	//    --------x         Y                    
	//                                   
	
	local float  fWidth, fHeight, fDepth;
	
	// Calculate the width, depth and height of our bbox
	fWidth  = bbArea.Max.x - bbArea.Min.x;
	fDepth  = bbArea.Max.y - bbArea.Min.y;
	fHeight = bbArea.Max.z - bbArea.Min.z;
	
	
	// Choose a side for the starting point, then use the opposite side for the ending point
	switch( rand(6) )
	{
		case 0: // left side -> right side
			vStartPoint = bbArea.min + vec(0, frand()*fDepth, frand()*fHeight); // left
			vEndPoint	= bbArea.max - vec(0, frand()*fDepth, frand()*fHeight); // right
			break;
		
		case 1: // back side -> front side
			vStartPoint = bbArea.min + vec(frand()*fWidth, 0, frand()*fHeight); // back
			vEndPoint	= bbArea.max - vec(frand()*fWidth, 0, frand()*fHeight); // front
			break;
		
		case 2: // bottom side -> top side
			vStartPoint = bbArea.min + vec(frand()*fWidth, frand()*fDepth, 0); // bottom
			vEndPoint	= bbArea.max - vec(frand()*fWidth, frand()*fDepth, 0); // top
			break;
		
		case 3: // right side -> left side 
			vStartPoint = bbArea.max - vec(0, frand()*fDepth, frand()*fHeight); // right
			vEndPoint	= bbArea.min + vec(0, frand()*fDepth, frand()*fHeight);	// left
			break;
		
		case 4: // front side -> back side
			vStartPoint = bbArea.max - vec(frand()*fWidth, 0, frand()*fHeight); // front
			vEndPoint	= bbArea.min + vec(frand()*fWidth, 0, frand()*fHeight);	// back
			break;
		
		case 5: // top side -> bottom side
			vStartPoint = bbArea.max - vec(frand()*fWidth, frand()*fDepth, 0); // top
			vEndPoint	= bbArea.min + vec(frand()*fWidth, frand()*fDepth, 0); // bottom
			break;
	}
	
	// Compute the cut length
	vCutLength = vEndPoint - vStartPoint;
	
}

function UpdateDiffindoFX( float fTimeDelta )
{
	local float fTravel;

	fCutTime += fTimeDelta;
	
	// find our travel scalar ( from 0 to 1 )
	fTravel = fCutTime / fSingleCutTimer;
	
	if( fTravel >= 1.0f )
	{
		fxCut.SetLocation( vEndPoint );

		// Compute new start and end points to cut with
		ComputeNewStartAndEndPoints();

		// Reset our cut Time for the next cut.
		fCutTime = 0.0f;

		// Make the cut time 10% faster after a single cut
		fSingleCutTimer *= 0.9;
		
		return;
	}
	
	// update the position of the hitFX so that it travels from its start point to its max point
	fxCut.SetLocation( vStartPoint + (vCutLength * fTravel ) );
}

function OnDiffindoExplode()
{	
	// Create Explode FX (up to 4 diffrent kinds )
	if( fxExplodeClass0 != None )
		fxExplode = spawn( fxExplodeClass0,[SpawnOwner] self, [SpawnLocation] location );	
	if( fxExplodeClass1 != None )
		fxExplode = spawn( fxExplodeClass1,[SpawnOwner] self, [SpawnLocation] location );	
	if( fxExplodeClass2 != None )
		fxExplode = spawn( fxExplodeClass2,[SpawnOwner] self, [SpawnLocation] location );	
	if( fxExplodeClass3 != None )
		fxExplode = spawn( fxExplodeClass3,[SpawnOwner] self, [SpawnLocation] location );	

	// Play Cut sound Spell SoundFX
	if( DiffindoCutSound != None )
		PlaySound( DiffindoCutSound, SLOT_None );
}

function Trigger( actor Other, pawn EventInstigator )
{
	//If we receive an event
	GotoState('stateHitByDiffindo');
}


// --------------------------------------------------------------------------------------------
// *** States

state auto stateIdle
{
	function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
	{	
		if( !bUseDiffindoSpellHitFX )
		{
			// clear out the particleEffectClass, thus making the spell not create hitFX
			spell.fxHitParticleEffectClass = None;
		}
		
		GotoState('stateHitByDiffindo');
		return true;
	}
}

state stateHitByDiffindo
{
	function BeginState()
	{
		// create the diffindo reaction
		fxCut = spawn( fxCutClass );	
		fxCut.SetLocation( location );
		fxCut.SetOwner( Self );

		ComputeNewStartAndEndPoints();
		
		fxCut.SetLocation( vStartPoint );
	
		// Play Impact sound Spell SoundFX
//		if( DiffindoImpactSound != None )
//			PlaySound( DiffindoImpactSound, SLOT_Interact,  1.0, false, 2000.0, 1);
	}

	function Tick( float fTimeDelta )
	{
		// Take time away from our diffindo timer
		fDiffindoTimer -= fTimeDelta;
		
		UpdateDiffindoFX( fTimeDelta );

		// Test to see if we are done with diffindo
		if( fDiffindoTimer < 0.0f )
		{
			OnDiffindoExplode();
			
			// Send out an event that we have been destroyied
			TriggerEvent( event, none, none );
		
			// Shutdown the Cut fx
			fxCut.Shutdown();

			// Destroy our diffindo object
			Destroy();
		}
	}
}


state stateHitByDiffindoNoFX
{
	function Tick( float fTimeDelta )
	{
		// Take time away from our diffindo timer
		fDiffindoTimer -= fTimeDelta;
		
		// Test to see if we are done with diffindo
		if( fDiffindoTimer < 0.0f )
		{
			// Send out an event that we have been destroyied
			TriggerEvent( event, none, none );
						
			// Destroy our diffindo object
			Destroy();
		}
	}
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- DiffindoObj
	fxCutClass=class'diffindo_fly'
	fxExplodeClass0=class'diffindo_hit'
	fxExplodeClass1=None
	fxExplodeClass2=None
	fxExplodeClass3=None

	fDiffindoTimer=3.0f
	fSingleCutTimer=0.3f

	bUseDiffindoSpellHitFX=false
	
	DiffindoImpactSound=Sound'HPSounds.magic_sfx.DFO_hit_rope'
	DiffindoCutSound=Sound'HPSounds.magic_sfx.DFO_hit_rope'

	// --- Character
	eVulnerableToSpell=SPELL_Diffindo

}



// --------------------------------------------------------------------------------------------
// HDiffindo.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------

