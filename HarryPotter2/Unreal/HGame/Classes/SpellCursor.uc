// --------------------------------------------------------------------------------------------
//   _____            _ _  _____                                          
//  / ____|          | | |/ ____|                                         
// | (___  _ __   ___| | | |     _   _ _ __ ___  ___  _ __     _   _  ___ 
//  \___ \| '_ \ / _ \ | | |    | | | | '__/ __|/ _ \| '__|   | | | |/ __|
//  ____) | |_) |  __/ | | |____| |_| | |  \__ \ (_) | |    _ | |_| | (__ 
// |_____/| .__/ \___|_|_|\_____|\__,_|_|  |___/\___/|_|   (_) \__,_|\___|
//        | |                                                             
//        |_|                                                             
// --------------------------------------------------------------------------------------------
// Class Name  : SpellCursor
//
// Created on  : 03/21/2002
// 
// Description : The spell cursor is the entity that indicates where you are aiming your spell.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpellCursor extends ParticleFX;

// --------------------------------------------------------------------------------------------
// *** Imports

#exec OBJ LOAD FILE=..\textures\SpellShapes.utx PACKAGE=SpellShapes.SpellFX


// --------------------------------------------------------------------------------------------
// *** Variables
// --- base
var Harry					playerHarry;			// ref to harry
var actor					aPossibleTarget;		// we may have a possible target/victim
var actor					aCurrentTarget;			// Our current victim (if locked on)


// --- Seeking								
var vector					vLOS_Dir;				// normalized direction from Harry to SpellCursor
var vector					vLOS_Start;				// Line Of Sight Start
var vector					vLOS_End;				// Line Of Sight End

var vector					vLastValidHitPos;			// Last Target Hit position

var	float					fLOS_Distance;			// Line of Sight distance
var	bool					bInvisibleCursor;		// If true then update the position of the cursor, but not the effects
var globalconfig bool		bSpellCursorAlwaysOn;	// if true then always show particles, seeking or not

var vector					vTargetOffset;			// Save our target offset so we don't have to get it again

var vector					vHitLocation;			// The location in space, where we hit our target
var vector					vHitNormal;				// The normal of the surface where we hit
var bool					bHitSomething;			// If our cursor is on something, anything at all, save that state


// --- locked on
var GestureSprite			SpellGesture;			// our spell gesture 
var vector					vGestureOffset;			// 
var float					fFinalGestureDistance;	// final gesture distance based on width/height of object

// --- Debug
var bool					bDebugMode;

// --------------------------------------------------------------------------------------------
// *** Constants
const MIN_GESTURE_SIZE		= 50.0f;
const MAX_GESTURE_SIZE		= 100.0f;


// --------------------------------------------------------------------------------------------
// *** Functions

function SetDebugMode( bool bOn )	{ bDebugMode = bOn; }


function bool IsLockedOn()			{ return aCurrentTarget != none; }

function PreBeginPlay()
{
	// Find harry and save him ( i'd like to do this diffrently )
	playerHarry = Harry(Level.playerHarryActor);

	if( !bSpellCursorAlwaysOn || bInvisibleCursor )
		EnableEmission( false );

	// spawn our spellGesture sprite
	SpellGesture = spawn( class'GestureSprite' );
	
	if( SpellGesture == none )
		playerHarry.ClientMessage(" Could not create Sprite SpellGesture!!! ");
}

function Destroyed()
{
	if( SpellGesture != None )
	{
		SpellGesture.Destroy();
	}
	Super.Destroyed();
}


function SetLOSDistance( float fNewDistance )
{
	if( fNewDistance == 0 )
		fNewDistance = default.fLOS_Distance;

	fLOS_Distance = fNewDistance;

	playerHarry.ClientMessage("SpellCursor: Set spell distance to " $fNewDistance );
}

function WetTexture GetGestureTexture( ESpellType SpellType )
{
	switch( SpellType )
	{
		case SPELL_None:			return None;
		case SPELL_Flipendo:		return WetTexture'SpellShapes.SpellFX.FlipendoWet1';
		case SPELL_Lumos:			return WetTexture'SpellShapes.SpellFX.LumosWet1';
		case SPELL_Alohomora:		return WetTexture'SpellShapes.SpellFX.AlohomoraWet1';
		case SPELL_Skurge:			return WetTexture'SpellShapes.SpellFX.SkurgeWet1';
		case SPELL_Rictusempra:		return WetTexture'SpellShapes.SpellFX.RictusWet1';
		case SPELL_Diffindo:		return WetTexture'SpellShapes.SpellFX.DiffindoWet1';
		case SPELL_Spongify:		return WetTexture'SpellShapes.SpellFX.SpongifyWet1';
	}
}

function TurnOnSpellGestureFX( ESpellType SpellType, vector vLocation, float fFXSize )
{
	// Error Checking
	if(aCurrentTarget == None)
		return;
	

	// Make sure our fFXSize is at least the minimum size
	if( fFXSize < MIN_GESTURE_SIZE )
		fFXSize = MIN_GESTURE_SIZE;
	else if( fFXSize > MAX_GESTURE_SIZE )
		fFXSize = MAX_GESTURE_SIZE;
	
	// set up our spell gesture sprite
	SpellGesture.SetLocation( vLocation );
	SpellGesture.SetRotation( playerHarry.Rotation );
	
	SpellGesture.texture		= GetGestureTexture( SpellType );
//	SpellGesture.bHidden		= false;
	SpellGesture.DrawScale		= 1.0f;

	//DEBUG
//	if(USE_DEBUG_MODE) playerHarry.ClientMessage("Created SpellFX gesture -> " $WinFX $" FXsize: " $fFXSize);
}

function bool CanCameraSeeYouInFOV( int rOutsideFOV, vector pos )
{
	local vector  normal, dir;
	local rotator OutsideFOV;
	local float	  fDotProduct;
	
	// compute dir vector
	dir	= pos - playerharry.cam.location;
	
	// If our target become farther than our LOS distance (plus a little cusion ) then we cant see it
	if( vsize(dir) > fLOS_Distance * 1.25f )
		return false;

	// compute normal vector
	OutsideFOV.yaw = playerHarry.cam.rotation.yaw - rOutsideFOV;
	normal = vector( OutsideFOV );
	
	// point to plane check
	if( (normal dot dir) > 0.0f )
	{
		// check a second plane ( to make up the FOV )

		// compute diffrent normal
		OutsideFOV.yaw = playerHarry.cam.rotation.yaw + rOutsideFOV;
		normal = vector( OutsideFOV );
		
		// point to plane check
		if( (normal dot dir) > 0.0f )
			return true; // we are inside the 2 planes (that make up the FOV)
	}
	
	return false;
}

// Update the cursor's position by following our LOS and see what we hit first
function UpdateCursor( optional bool bJustStopAtClosestPawnOrWall )
{
	local actor		HitActor;
	local bool		bHitActor;
	local vector	vFirstHitPos;

	// If we are not currently targeting then return
	if( bEmit == false && !bInvisibleCursor )
		return;
	
	// Reset our possible victim
	aPossibleTarget = none;
	bHitSomething	= false;
	
	// *** Set our LOS (line of sight) START point
	// (obtained from the player's location + offset to harry's eyes)
	vLOS_Start	= playerHarry.cam.CamTarget.location;//playerHarry.location + vec(0,0,playerHarry.EyeHeight);
	
	// *** Set our LOS END point
	if( playerHarry.bInDuelingMode )
	{
		vLOS_End = playerHarry.Location
		          + (vector(playerHarry.rotation) * fLOS_Distance);
	}
	else
	if( playerHarry.bHarryUsingSword )
	{
		vLOS_End =  playerHarry.cam.Location
		          + ( vector(playerHarry.cam.rotation+playerHarry.AimRotOffset) * (playerHarry.cam.CurrentSet.fLookAtDistance + fLOS_Distance) );
	}
	else
	{
		// (obtained by the camera's forward vector)
		// our line of sight END point is deturmined by the forward vector of the camera
		vLOS_End = playerHarry.cam.Location + 
			( playerHarry.cam.vForward * (playerHarry.cam.CurrentSet.fLookAtDistance + fLOS_Distance) );
	}

	// *** Set our LOS direction vector
	vLOS_Dir = normal(vLOS_End - vLOS_Start);
	
	// The trace line is from the camera's pos allong it's forward vector
	// we do this so we can find the point that Harry is aiming at
	HitActor = Trace(vHitLocation, vHitNormal, vLOS_End, playerHarry.cam.Location );

	if( HitActor != None && !HitActor.IsA('BaseHarry') )
	{
		// We hit a wall
		bHitSomething = true;
		
		// Set our new end point 
		// (add a small extension so if we hit an actor the next line check from harry to the end point will find it)
		vLOS_End = vHitLocation + (vLOS_Dir * 5.0);
	}
	
	// *** Check LOS collision with actors
	// test the line segment between harry and the possible target
	foreach TraceActors(class'actor', HitActor, vHitLocation, vHitNormal, vLOS_End, vLOS_Start)
	{
		// *** Hit actor, update end point and see if it is a potential target

		// Trivially reject objects
		if( HitActor == Owner || HitActor.IsA('Harry') ||
			( !HitActor.IsA('Pawn') && !HitActor.IsA('GridMover') && !HitActor.IsA('spellTrigger')) )
			continue;

		//DEBUG
		if( bEmit && bDebugMode )
			playerHarry.ClientMessage(" TraceActors Hit actor -> " $HitActor );
		
		// Save our first hitActor if it is visible
		if(!bHitActor && !HitActor.bHidden )
		{
			// We hit something ( although it may not be a valid target to lock onto )
			bHitSomething = true;
			bHitActor     = true;
			vFirstHitPos  = vHitLocation;
		}
		
		if( HitActor.eVulnerableToSpell == SPELL_None )
			continue;
		
		// --- See if we have a possible target
		if( playerHarry.IsInSpellBook( HitActor.eVulnerableToSpell ) || ( bJustStopAtClosestPawnOrWall ) )
		{
			// If we hit a spell trigger make sure it can be hit at this time
			if( HitActor.IsA('spellTrigger') && !spellTrigger(HitActor).bInitiallyActive )
				continue;
			

			// We found a possible target, if doing normal casting, set some vars, then break out
			if( !bJustStopAtClosestPawnOrWall )
			{
				aPossibleTarget = HitActor;
				vTargetOffset   = vHitLocation - aPossibleTarget.location;
			}
	
			// Weather it was a valid target or not we need to leave now that we hit our first object
			vLastValidHitPos = vHitLocation;
		}
		
		vLOS_End = vHitLocation;
		break;
	
	} // end for each actor
	
	// If we hit an actor but we didn't find a possible target then set our new end point at our first hitLocation
	if( aPossibleTarget == None && bHitActor )
		vLOS_End = vFirstHitPos;
	
	// *** Update Position 
	if( aCurrentTarget == None )
	{
		// Move our target to point where our LOS ends offset by a little bit
		MoveSmooth((vLOS_End - (vLOS_Dir * 8.0)) - Location);

		if( aPossibleTarget != None )
			SpellGesture.SetLocation( vLOS_End );
	}

}

// Assuming we have called Update Position lets see if we can find a target
function bool LookForTarget()
{
	if( aPossibleTarget == None )
	{	
		return false; // we don't have a target
	}
	
	// *** If we have the same target
	if( aPossibleTarget == aCurrentTarget )
	{	
		return true; // We are still on the same target so return true
	}

	// SO FAR WE KNOW THAT: (aPossibleTarget != None) && (aPossibleTarget != aCurrentTarget)
		
	// Lock onto our new possible target
	LockOn( aPossibleTarget );
	return true;
}

function UnLock()
{
	if( aCurrentTarget == none )
		return;
	
	// Stop our spell sound loop
	StopLockedOnSoundLoop();
	
	// Reset our vars
	aPossibleTarget	= None;
	aCurrentTarget	= None;
	
	// hide our spell gesture
	SpellGesture.bHidden = true;
}

function LockOn( actor TargetActor )
{
	local float		fTargetWidth;
	local float		fTargetHeight;
	local float		fTargetDepth;
	local vector	dwh;
	
	// Get our target's height, width and depth
	if( TargetActor.CollideType == CT_AlignedCylinder  || TargetActor.CollideType == CT_OrientedCylinder || TargetActor.CollisionWidth == 0)
		dwh = vec( TargetActor.CollisionRadius, TargetActor.CollisionRadius, TargetActor.CollisionHeight );
	else
		dwh = vec( TargetActor.CollisionRadius, TargetActor.CollisionWidth, TargetActor.CollisionHeight  );
	
	// Create sparkels that have a width and height == to the Target's h,d,w with a small cusion to make it larger
	fTargetDepth  = dwh.x * 2.2f * TargetActor.SizeModifier; // depth
	fTargetWidth  = dwh.y * 2.2f * TargetActor.SizeModifier; // width
	fTargetHeight = dwh.z * 2.2f * TargetActor.SizeModifier; // height
	
	
	// error checking
	if( TargetActor == none || playerHarry == none || playerHarry.Weapon == none )
	{
		log("SpellCursor::LockOn() -> ERROR TargetActor or playerHarry or playerHarry.Weapon is invalid!!!");
		return;
	}
	
	// We will choose the gesture's dist from center based upon what is larger (width or depth)
	if(fTargetDepth < fTargetWidth)
		fFinalGestureDistance = (fTargetDepth*0.5f) + 2.0f + TargetActor.GestureDistance;
	else
		fFinalGestureDistance = (fTargetWidth*0.5f) + 2.0f + TargetActor.GestureDistance;
	
	// Tell our wand what spell we wish to use
	basewand(playerHarry.Weapon).ChooseSpell( TargetActor.eVulnerableToSpell );

	// Save our TargetActor as our CurrentVictim
	aCurrentTarget = TargetActor;
	
	if( aCurrentTarget.bGestureFaceHorizOnly )
	{
		// The gesture will be placed at the location of our target then offsetted toward harry
		// by this vector -> vGestureOffset.
		vGestureOffset = -(vec(fFinalGestureDistance, 0, 0));
		
		// **** Setup our Spell FX gesture
		// Create our SpellFX (gesture) use width or height for size, depending upon what is smaller
		TurnOnSpellGestureFX( TargetActor.eVulnerableToSpell,
							  TargetActor.location + TargetActor.CentreOffset + ( vGestureOffset >> playerHarry.rotation ), 
							  fFinalGestureDistance * TargetActor.SizeModifier );
	}
	else
	{
		// Have the gesture face Harry Horizontally and Vertically.
		vGestureOffset = normal(playerHarry.location - aCurrentTarget.location) * fFinalGestureDistance;
		
		TurnOnSpellGestureFX( TargetActor.eVulnerableToSpell, vLOS_End, 
						  fFinalGestureDistance * TargetActor.SizeModifier );
	}
	
	//DEBUG
	if(bDebugMode) playerHarry.ClientMessage("LockedOnto Target using Depth:" $fTargetDepth
		$" Width:" $fTargetWidth $" Height:" $fTargetHeight  );
	
	// Set up our locked on sparkles
	SetSparklesLockedOn( fTargetWidth, fTargetHeight, fTargetDepth );
	SetRotation( TargetActor.rotation );

	// Start our spell sound loop (while we aim)
	StartLockedOnSoundLoop();
	
}

//*** SOUND ***
function StartLockedOnSoundLoop()
{
	// Kick off starting sound for when we lock on.
	PlaySound(sound'HPSounds.magic_sfx.spell_target_nl3', SLOT_Misc);
	
	// Trigger the loop.
	PlaySound(sound'HPSounds.magic_sfx.spell_targetloop', SLOT_Interact);
}

function StopLockedOnSoundLoop()
{
	// Trigger end sample.
//	PlaySound(sound'HPSounds.magic_sfx.spell_off_target3', SLOT_Misc);
	
	// Stop the effect loop.
	StopSound(sound'HPSounds.magic_sfx.spell_targetloop', SLOT_Interact);
}


//*** SPARKELS ***

function TurnSparklesOff()
{

}

function SetSparklesIdle()
{
	// Setup our spell cursor particles
	ParticlesPerSec.Base		= 20.0f;
	SourceWidth.Base			= 3.0f;
	SourceHeight.Base			= 3.0f;
	SourceDepth.Base			= 3.0f;
	speed.Base					= 0.0f;
	Lifetime.Base				= 0.3f;
								
	SizeWidth.Base				= 4.0f;
	SizeLength.Base				= 4.0f;
	SizeEndScale.Base			= 0.75f;
	SpinRate.Base				= 4.0f;
	SpinRate.Rand				= -8.0f;
	ParticlesAlive				= 10;
								
	ColorStart.Base.R			= 255;
	ColorStart.Base.G			= 0;
	ColorStart.Base.B			= 0;
								
	ColorEnd.Base.R				= 255;
	ColorEnd.Base.G				= 255;
	ColorEnd.Base.B				= 255;
}

function SetSparklesSeeking()
{
	// Setup our spell cursor particles
	ParticlesPerSec.Base		= 20.0f;
	SourceWidth.Base			= 10.0f;
	SourceHeight.Base			= 10.0f;
	SourceDepth.Base			= 10.0f;
	AngularSpreadWidth.Base		= 2.0f;
	AngularSpreadHeight.Base	= 2.0f;
	speed.Base					= 5.0f;
	Lifetime.Base				= 2.0f;
	
	SizeWidth.Base				= 8.0f;
	SizeWidth.Rand				= 10.0f;
	SizeLength.Base				= 8.0f;
	SizeLength.Rand				= 10.0f;
	SizeEndScale.Base			= -0.5f;
	SpinRate.Base				= 1.0f;
	SpinRate.Rand				= 20.0f;
	Attraction.X				= 10.0f;
	Attraction.Y				= 10.0f;
	ParticlesAlive				= 10;

	ColorStart.Base.R			= 255;
	ColorStart.Base.G			= 255;
	ColorStart.Base.B			= 255;
								
	ColorEnd.Base.R				= 255;
	ColorEnd.Base.G				= 255;
	ColorEnd.Base.B				= 0;
}

function SetSparklesLockedOn( float fTargetWidth, float fTargetHeight, float fTargetDepth )
{
	playerHarry.cm("SetSparklesLockedOn -> fTargetWidth=" 
		$fTargetWidth $" fTargetHeight=" $fTargetHeight $" fTargetDepth=" $fTargetDepth );

	// Setup our spell cursor particles
	ParticlesPerSec.Base	= 60.0;
	SourceWidth.Base		= fTargetWidth;
	SourceHeight.Base		= fTargetHeight;
	SourceDepth.Base		= fTargetDepth;
	AngularSpreadWidth.Base	= 2.0;
	AngularSpreadHeight.Base= 2.0;
	speed.Base				= 5.0;
	Lifetime.Base			= 2.0;

	SizeWidth.Base			= 2.0;
	SizeWidth.Rand			= 10.0;
	SizeLength.Base			= 2.0;
	SizeLength.Rand			= 10.0;
	SizeEndScale.Base		= -0.5;
	SpinRate.Base			= 1;
	SpinRate.Rand			= 20.0;
	Attraction.X			= 10.0;
	Attraction.Y			= 10.0;

	ParticlesAlive			= 30;
	bRotateToDesired		= true;

	ColorStart.Base.R		= 255;
	ColorStart.Base.G		= 0;
	ColorStart.Base.B		= 255;

	ColorEnd.Base.R			= 255;
	ColorEnd.Base.G			= 0;
	ColorEnd.Base.B			= 255;
}


//*** TARGETING ON/OFF ***
function TurnTargetingOn()
{
	// suppose to be empty
	//
	// startSeeking does something only if you are in the Idle state
	//

	//DEBUG
//	playerHarry.ClientMessage("!!! CALLED TurnOn() when not in StateIdle()!!!");
}

function TurnTargetingOff()
{
	// Get rid of our lock (if we had any)
	UnLock();
	
	// Goto an Idle state
	GotoState('StateIdle');
}

// --------------------------------------------------------------------------------------------
// *** States
state auto stateIdle
{
	function BeginState()
	{
		// setup our idle sparkles
		SetSparklesIdle();

		// If you only want to see particles when you are casting a spell
		if( !bSpellCursorAlwaysOn || bInvisibleCursor )
			EnableEmission( false );
	}
	
	function Tick( float fTimeDelta )
	{
		// Update our Cursor's LOS and position
		UpdateCursor( playerHarry.bHarryUsingSword );
		
		// Update the color depending upon weather or not we are on something
		if( bHitSomething )
		{
			ColorStart.Base.R	= 255;
			ColorStart.Base.G	= 255;
			ColorStart.Base.B	= 0;
		}
		else
		{
			ColorStart.Base.R	= 255;
			ColorStart.Base.G	= 0;
			ColorStart.Base.B	= 0;
		}
	}

	function TurnTargetingOn()
	{
		// You can turn on and off the Spell target, like a flashlight.
		GotoState('StateSeeking');
	}

	begin:
	if(bDebugMode) playerHarry.ClientMessage("BeginState -> StateIdle");
}

state stateSeeking
{
	function BeginState()
	{
		// If you only want to see particles when you are casting a spell
		if( !bInvisibleCursor )
			EnableEmission( true );
		
		// setup our seeking sparkles
		SetSparklesSeeking();
	}

	function EndState()
	{

	}
	
	function Tick( float fTimeDelta )
	{
		// Update our Cursor's LOS and position
		UpdateCursor( playerHarry.bHarryUsingSword );
		
		// Update the color depending upon weather or not we hit something
		if( bHitSomething )
		{
			ColorStart.Base.R	= 255;
			ColorStart.Base.G	= 255;
			ColorStart.Base.B	= 0;
		}
		else
		{
			ColorStart.Base.R	= 255;
			ColorStart.Base.G	= 0;
			ColorStart.Base.B	= 0;
		}
		
		// Look for a possible target
		if( true == LookForTarget() )
		{
			// we found a target so lets lock on
			GotoState('StateLockedOn');
		}
	}

	begin:
	if(bDebugMode) playerHarry.ClientMessage("BeginState -> StateSeeking");
}

state stateLockedOn
{
	function Tick( float fTimeDelta )
	{
		// Update our Cursor's LOS and position
		UpdateCursor();
		
		// If harry can still see his target, stay locked on, otherwise unlock.
		//
		// Pre caculate the rot value to rotate for speed purposes.
		// --------------------------------------------------
		// FOV		outsideFOV		formula to determine the rotator value for left and right outsideFOV
		// 140		180-140=40		(40/360) * 0xFFFF =  7281
		// 130		180-130=50		(50/360) * 0xFFFF =  9102
		// 120		180-120=60		(60/360) * 0xFFFF = 10923
		// 110		180-110=70		(70/360) * 0xFFFF = 12743
		// 100		180-100=80		(80/360) * 0xFFFF = 14563
		
		if( !LookForTarget() && !CanCameraSeeYouInFOV( 10923, aCurrentTarget.location ) )
		{
			UnLock();
			GotoState('StateSeeking');
		}
		
		// *** Update the position of our cursor so that its always in the middle of our target
		SetLocation( aCurrentTarget.location );		
		
		// Set our Spell Gesture's location at its last hit position then add a little cusion toward harry
		if( aCurrentTarget != None && aCurrentTarget.bGestureFaceHorizOnly )
		{
			// Spell location = center of target offset by the depth of the target then rotated by harry's current rotation
			SpellGesture.SetLocation( aCurrentTarget.location + aCurrentTarget.CentreOffset + 
				(vGestureOffset >> playerHarry.Rotation) );
			SpellGesture.bHidden = false;
		}
		else 
		if( aPossibleTarget != None )
		{
			if( SpellGesture.bHidden && aPossibleTarget == aCurrentTarget )
			{
				// Spell gesture is going to warp to hit location because we JUST found a target
				SpellGesture.SetLocation( vLOS_End );
				SpellGesture.bHidden = false;
			}
			else
			{
				// Already had this target
				SpellGesture.MoveSmooth( (vLOS_End - SpellGesture.Location) * 10.0f * fTimeDelta );
			}
			vTargetOffset = SpellGesture.Location - Location;
		}
		else
		if( aCurrentTarget != None )
		{
			// Spell gesture is going back to target's location

			vGestureOffset = normal(playerHarry.location - aCurrentTarget.location) * fFinalGestureDistance;						
			SpellGesture.MoveSmooth( ((aCurrentTarget.location + aCurrentTarget.CentreOffset + vGestureOffset) - SpellGesture.Location) * 8.0f * fTimeDelta );
			vTargetOffset = SpellGesture.Location - Location;
		}

		
		// Update the color depending upon weather or not we hit something
		if( bHitSomething || aCurrentTarget != None )
		{
			ColorStart.Base.R	= 255;
			ColorStart.Base.G	= 255;
			ColorStart.Base.B	= 0;
		}
		else
		{
			ColorStart.Base.R	= 255;
			ColorStart.Base.G	= 0;
			ColorStart.Base.B	= 0;
		}		
	}
	
	begin:
	if(bDebugMode) playerHarry.ClientMessage("BeginState -> StateLockedOn ( " $aCurrentTarget $" )");
}

// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// --- SpellCursor

	// The default distance between harry and the target is 512
	fLOS_Distance=512
	bSpellCursorAlwaysOn=false
	
	bDebugMode=false
	
	// --- ParticleFX
	bStatic=False
	bblockactors=false
	bblockplayers=false
	bcollideactors=false
	bcollideworld=false
	bprojtarget=false
	ParticlesPerSec=(Base=20.0f)
	SourceWidth=(Base=100.0f)
	SourceHeight=(Base=100.0f)
	SourceDepth=(Base=100.0f)
	AngularSpreadWidth=(Base=2.0f)
	AngularSpreadHeight=(Base=2.0f)
	speed=(Base=5.0f)
	Lifetime=(Base=2.0f)
	ColorStart=(Base=(R=0,G=0,B=255))
	ColorEnd=(Base=(R=0,G=0,B=255))
	SizeWidth=(Base=2.0f,Rand=10.0f)
	SizeLength=(Base=2.0f,Rand=10.0f)
	SizeEndScale=(Base=-0.500000)
	SpinRate=(Base=1.0f,Rand=20.0f)
	Attraction=(X=10.0f,Y=10.0f)
	ParticlesAlive=10
	Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_1'
	Rotation=(Pitch=16640)
	bRotateToDesired=True
}

// --------------------------------------------------------------------------------------------
// SpellCursor.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------