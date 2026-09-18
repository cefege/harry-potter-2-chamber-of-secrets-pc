// --------------------------------------------------------------------------------------------
//   _____                         _  __       _____            _                
//  / ____|                       (_)/ _|     |  __ \          | |               
// | (___  _ __   ___  _ __   __ _ _| |_ _   _| |__) | __ _  __| |    _   _  ___ 
//  \___ \| '_ \ / _ \| '_ \ / _` | |  _| | | |  ___/ / _` |/ _` |   | | | |/ __|
//  ____) | |_) | (_) | | | | (_| | | | | |_| | |    | (_| | (_| | _ | |_| | (__ 
// |_____/| .__/ \___/|_| |_|\__, |_|_|  \__, |_|     \__,_|\__,_|(_) \__,_|\___|
//        | |                 __/ |       __/ |                                  
//        |_|                |___/       |___/                                   
// --------------------------------------------------------------------------------------------
// Class Name  : SpongifyPad
//
// Created on  : 06/04/2002
// 
// Description : Spongify pads will be present throughout the game, represented by a unique 
//				 and recognizable texture on the floor. To activate the “sponginess”, the spell 
//				 must be cast directly at the texture. Once the pad is active, Harry can use it 
//				 as a super jump pad, possibly having two levels of height triggered by the number 
//				 of jumps on the pad.
//				 The designer can have a Spongify Target to aim for. To determine what target object 
//				 goes to what pad, the event of the pad must == the tag of the target object.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpongifyPad extends HProp;

#exec Texture Import File=Textures\Spongy.pcx Name=SpongifyTexture Mips=Off Flags=2

// --------------------------------------------------------------------------------------------
// *** Variables

var actor				Target;				// Target that this pad is going to aim for (if it has a target)
var vector				vStartPosition;
var rotator				rLast;
var float				fScale;

var() bool				bEnableOnEvent;		// if true don't allow enabling of spell untill you recieve an event

var float				fTimeLeft;
var() float				fTimeEnabled;		// Time to keep the spongifyPad enabled (after a spongify spell hit)

var() float				fRaiseAmount;		// amount to raise from floor

var() float				fTimeToHitTarget;	// If there is a target how long do you wait to hit it?
var() vector			PadDir;				// Spongify Pad's jump direction
var() float				PadSpeed;			// Spongify Pad's jump speed

var()ParticleFX			fxSparkles;			// Sparkles that come from the pad
var()class<ParticleFX>	fxSparklesClass;	// Class of particleFX used to create the sparkles

var SpongifySheet		fxSheet;			// the quad texture used for drawing the "pad" itself
var bool				bBouncing;			// Is the pad doing a bouncing effect?

var()float				fLogTimeInc;		// pre production tool: log position at time inc

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions

// helper functions
function float ConvertDegToRot	( float fDeg )		{ return (fDeg / 360) * 65536; }
function color Col(float R, float G, float B)
{
	local color C;
	C.R = R;  C.G = G;  C.B = B;
	return C;
}


function PreBeginPlay()
{
	local float  time;
	local vector vel;

	Super.PostBeginPlay();
	playerHarry = Harry(Level.playerHarryActor);

	// Set our collision to collide actors = true, block actors or players == false
	SetCollision(,,);
	
	// See if we have a target
	if(event != '')
	{
		foreach AllActors( class'actor', Target )
		{
			// Check to see if this is our Target
			if( event == Target.tag )
				break;
		}
	}

	// If we have a target then compute the correct direction and speed
	if( Target != None )
	{
		vel		 = ComputeTrajectoryByTime( location, Target.location, fTimeToHitTarget );
		PadSpeed = vsize (vel);
		PadDir	 = normal(vel);
	}
	vStartPosition = location;


/*	// COMMENTED OUT FOR LATER USE

	// ***********************
	// Output the future points in space harry will travel during his bounce
	// this is done so we can project the futore
	time = 0.0f;

	log("***** SpongifyPad " $self $" *****" );
	log("" $self $" Time: " $time $" Loc: " $location );	
	do
	{
		// update time by fLogTimeInc
		time += fLogTimeInc;
		
		// log future time and loc
		log("" $self $" Time: " $time $" Loc: " $ProjectFuturePosition( time ) );
	}until( time > fTimeToHitTarget );
	// ***********************
*/
}

function vector ProjectFuturePosition( float AtTime )
{
	return location + ( (PadDir * PadSpeed) * AtTime ) + ( Region.Zone.ZoneGravity * AtTime );
}

event Destroyed()
{
	TurnOffSpecialFX();

	Super.Destroyed();
}

function bool IsEnabled()
{
	return !IsInState('stateDisabled');
}


function TurnOnSpecialFX()
{
	local vector hwd, hwdRotated;
	
	// *** Create Sheet
	if( fxSheet == None )
	{
		fxSheet = spawn( class'SpongifySheet',[SpawnOwner]self, [SpawnLocation]location );
		fxSheet.DesiredRotation = rot(0,0,0);
		fxSheet.DesiredRotation.yaw += 16383;
		fxSheet.SetRotation( fxSheet.DesiredRotation );
	}

	// *** Create Sparkles
	if( fxSparkles == None )
	{
		// rotate our collision d,w,h so that it corisponds with rotated Areas
		if( CollideType == CT_AlignedCylinder || CollideType == CT_OrientedCylinder || CollisionWidth == 0)
			hwd = vec( CollisionRadius, CollisionRadius, CollisionHeight );
		else
			hwd = vec( CollisionRadius, CollisionWidth, CollisionHeight  );
		hwdRotated = hwd >> rotation;
		
		fxSparkles = spawn( fxSparklesClass,[SpawnOwner]self, [SpawnLocation]location );
		
		// Create sparkels that have a width and height == to the collision bbox	
		fxSparkles.SourceDepth.Base		= hwdRotated.x * 2.0f; // depth
		fxSparkles.SourceWidth.Base		= hwdRotated.y * 2.0f; // width
		fxSparkles.SourceHeight.Base	= hwdRotated.z * 1.0f; // height
		
		// make the particlesPerSec based on the speed of our pad!
/*		fxSparkles.ParticlesPerSec.Base	= vsize(hwd) * PadSpeed * 0.001f;
		fxSparkles.Lifetime.Base    = 1.5f;
		fxSparkles.Lifetime.Rand    = 0.5f;
		fxSparkles.Chaos			= 0.0f;
		fxSparkles.Speed.Base		= 0;
		fxSparkles.Speed.Rand		= 0;
		fxSparkles.GravityModifier	= 0.07f;
		fxSparkles.Gravity			= -PadDir;
		fxSparkles.ColorStart.Base	= col(236,17,220);
		fxSparkles.ColorStart.Rand	= col(211,12,202);
		fxSparkles.ColorEnd.Base	= col(254,1,254);
		fxSparkles.ColorEnd.Rand	= col(243,1,189);
*/
	}
}

function TurnOffSpecialFX()
{
	if( fxSparkles != None )
	{
		fxSparkles.Shutdown();
		fxSparkles = None;
	}

	if( fxSheet != None )
	{
		fxSheet.Destroy();
		fxSheet = None;
	}
}

function UpdateSpecialFX( float fTimeDelta )
{
	// *** Update SpecialFX
	// Update sparkles
	fxSparkles.SetRotation( rotation );
	fxSparkles.SetLocation( location );
	
	// Update sheet
	fxSheet.DesiredRotation = rotation;
	fxSheet.DesiredRotation.yaw += 16383;
	fxSheet.SetRotation( fxSheet.DesiredRotation );
	fxSheet.SetLocation( location );
	
	// Update bouncing effect
	if( bBouncing )
	{
		if( fxSheet.DrawScale > fxSheet.default.DrawScale )
			fxSheet.DrawScale -= 4 * fTimeDelta;
		else
		{
			// Go back to our regular scale
			fxSheet.DrawScale  = fxSheet.default.DrawScale;
			bBouncing = false;
		}
	}
}


function OnBounce( actor other )
{
	playerHarry.ClientMessage( " ONBounce called " $other );
	
	// If the object that hit us is falling and has a negitave z velocity we should bounce it!
	if( other.IsA('Harry') )
	{
		if( Target != None )
			other.Velocity = ComputeTrajectoryByTime( location, Target.location, fTimeToHitTarget );
		else
			other.Velocity = PadDir * PadSpeed;
		
		fxSheet.DrawScale = fxSheet.default.DrawScale * 2;
		bBouncing = true;
		
		// Play spongify soundFX
		PlaySound( Sound'HPSounds.Magic_sfx.SPN_bounce_on', SLOT_None, , true);
	}
}

function Trigger( actor Other, pawn EventInstigator )
{
	//If we receive an event grow or shrink depending upon the ectoplasma settings
	GotoState('stateDisabled');
}

// --------------------------------------------------------------------------------------------
// *** States
state auto stateWaitForEvent
{
	function BeginState()
	{
		// if we are waiting for an event then
		if( !bEnableOnEvent )
			GotoState('stateDisabled');
	}
}

state stateDisabled
{
	function BeginState()
	{
		//reset our vulnerableToSpell enum
		eVulnerableToSpell = SPELL_Spongify;
		bHidden=true;
		DesiredRotation = default.rotation;
		SetRotation(DesiredRotation);
		TurnOffSpecialFX();
	}
	
	function bool HandleSpellSpongify( optional baseSpell spell, optional vector vHitLocation )
	{
		GotoState('stateGoingToEnabled');
		return true; // this is a valid hit
	}
}

state stateGoingToEnabled
{
	function BeginState()
	{
		local vector  vNewPos;
		local rotator rNewRot;

		// This spongifyPad is no longer vulnerable to the spongify spell
		eVulnerableToSpell=SPELL_None;
		
		// Turn on the spongify sheet and particles 
		TurnOnSpecialFX();
		
		// Set our desired rotation
		rNewRot = rotator( PadDir );
		DesiredRotation.yaw = rNewRot.yaw;
		
		// Format our desired pitch (Limit to 10 degree angles)
//		if( DesiredRotation.pitch > ConvertDegToRot(10) )
//			DesiredRotation.pitch = ConvertDegToRot(10);
//		DesiredRotation.pitch = -DesiredRotation.pitch;
		
		rLast = DesiredRotation;

		// Play spongify soundFX
		PlaySound( Sound'HPSounds.Magic_sfx.SPN_activate', SLOT_None, , true);

		SetLocation( vStartPosition + vec(0,0, fRaiseAmount) );
		fxSheet.SetLocation( location );
	}
	
	function Tick( float fTimeDelta )
	{
		local float fNewZ;
		
		// *** Update position
//		if( fxSheet.location.z < vStartPosition.z + fRaiseAmount )
//			fxSheet.SetLocation( vStartPosition + vec(0,0, fRaiseSpeed * fTimeDelta ) );
//		else
//			fxSheet.SetLocation( vStartPosition + vec(0,0, fRaiseAmount) );
		
//		fNewZ = (vec(CollisionRadius, CollisionWidth, CollisionHeight ) << rotation).z;
//		if( location.z < vStartPosition.z + fNewZ )
//			SetLocation( vStartPosition + vec(0,0,fNewZ) );
		
		// *** Update SpecialFX
		UpdateSpecialFX( fTimeDelta );
		
		// I'm not getting the EndedRotation() event, and after looking into the 
		// engine code it looks like the == comparision is literal, 
		// and doesn't take into account the number of rotations or how a 
		// negitave rotation can equal a positive rotation (as in it faces the same dir).
		//
		// So to compensate for these shortcommings, (to find out when we are done rotating)
		// I test the current rotation with the last rotation, (instead of rotation == desiredRot)
		// if they are == then we are done.
		// 
		if( rLast == rotation && fxSheet.location.z == vStartPosition.z + fRaiseAmount )
			GoToState('stateEnabled');
		rLast = rotation;
	}


}

state stateEnabled
{
	function BeginState()
	{
		fTimeEnabled = 0;
	}
	
	function Tick( float fTimeDelta )
	{	
		fTimeEnabled += fTimeDelta;

		// *** Update SpecialFX
		UpdateSpecialFX( fTimeDelta );
	
//		if( fxSheet.location.z < vStartPosition.z + fRaiseAmount )
//			fxSheet.SetLocation( vStartPosition + vec(0,0, fRaiseSpeed * fTimeDelta ) );
//		else
//			fxSheet.SetLocation( vStartPosition + vec(0,0, fRaiseAmount) );
	
		// *** Update position
		if( fTimeEnabled > default.fTimeEnabled )
		{
			//If we ran out of time then begin disabling our pad
			GotoState('stateGoingToDisabled');
		}

//		fxSheet.Opacity = 1.0 - (fTimeEnabled/default.fTimeEnabled);
//		playerharry.clientmessage("opacity = " $fxSheet.opacity );
	}
}

state stateGoingToDisabled
{
	function BeginState()
	{
		DesiredRotation = rot(0,0,0);
		
		SetLocation( vStartPosition );
		fxSheet.SetLocation( location );
	}
	function Tick( float fTimeDelta )
	{
		// *** Update SpecialFX
		UpdateSpecialFX( fTimeDelta );
		
/*		if( fxSheet.location.z > vStartPosition.z )
			fxSheet.SetLocation( vStartPosition - vec(0,0, fRaiseSpeed * fTimeDelta ) );
		else
			fxSheet.SetLocation( vStartPosition );
*/
		if( rLast == rotation )
			GoToState('stateDisabled');
		rLast = rotation;
	}
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- Spongify Pad
	fTimeEnabled=30.0f
	fTimeToHitTarget=2.0f
	PadSpeed=500.0f
	PadDir=(X=0,Y=0,Z=1.0f)
	
	fRaiseAmount=10

	fxSparklesClass=class'spongifyFlash'
	
	// --- HPawn
	eVulnerableToSpell=None
	
	Physics=PHYS_Rotating
	bRotateToDesired=true
	RotationRate=(Yaw=50000,Pitch=50000,Roll=50000)
	
	bHidden=true
	DrawType=DT_Sprite
	Texture=Texture'HGame.SpongifyTexture'
	Mesh=None

	bCollideWorld=false
	bCollideActors=true
	bBlockActors=false
	bBlockPlayers=false
	
	CollideType=CT_Box
	CollisionWidth=48
	CollisionRadius=48
	CollisionHeight=4

	fLogTimeInc=0.1f
}

// --------------------------------------------------------------------------------------------
// SpongifyPad.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------

