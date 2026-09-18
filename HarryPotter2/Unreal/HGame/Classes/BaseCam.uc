// --------------------------------------------------------------------------------------------
//  _                     _____                                
// | |                   / ____|                               
// | |__   __ _ ___  ___| |      __ _ _ __ ___      _   _  ___ 
// | '_ \ / _` / __|/ _ \ |     / _` | '_ ` _ \    | | | |/ __|
// | |_) | (_| \__ \  __/ |____| (_| | | | | | | _ | |_| | (__ 
// |_.__/ \__,_|___/\___|\_____|\__,_|_| |_| |_|(_) \__,_|\___|
//                                                             
//                                                             
// --------------------------------------------------------------------------------------------
// Class Name  : baseCam
//
// Created on  : 03/28/2002
// 
// Description : The base cam has the implementation for all the standard camera modes
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------
class baseCam extends HPawn;


// --- Camera types
enum ECamMode
{
	CM_Startup,
	CM_Idle,
	CM_Transition,

	CM_Standard,
	CM_Quidditch,
	CM_FlyingCar,
	CM_Dueling,
	CM_CutScene,
	CM_Boss,

	CM_Free,
};

// --- Camera settings struct
struct CamSettings
{
	var vector		vLookAtOffset;		// Current Offset applyed to our lookAt point
	var float		fLookAtDistance;	// How far away from the lookAt point is the camera?
	var float		fRotTightness;		// The higher the tighness the faster our curLocation == targetLocation
	var float		fRotSpeed;			// How fast are we (per sec) at moving.
	var float		fMoveTightness;		// The higher the tighness the faster our curRotation == targetRotation
	var float		fMoveSpeed;			// How fast are we (per sec) at rotating.
};

// --------------------------------------------------------------------------------------------
// *** Variables

// --- Mouse Input data ( should i have to track this? )
var float			fMouseDeltaX;				// saved mouse deltaX and deltaY playerHarry
var float			fMouseDeltaY;	

// --- Base Cam data (used by all camera modes )
var ECamMode		CameraMode;					// Current Camera Mode
var ECamMode		CameraModeTransition;		// Camera Mode We are Transitioning to


var bool			bSyncRotationWithTarget;	// Sync our curr rotation to always face target
var bool			bSyncPositionWithTarget;	// Sync cam position with CamTarget's position at a fixed distance

var bool			bIgnoreTarget;				// You can choose to ignore the target when doing a flyto cutcommand

var BaseCamTarget   CamTarget;					// camera's target (seperate pawn so we can use flyto interpolation )
var vector			vForward;					// The forward vector (for refrence outside the camera)
var rotator			rRotationStep;				// Rotation Step ( rotation step will be applied over time)

var rotator			rDestRotation;				// Rotation Destination
var vector			vDestPosition;				// Position Destination
var rotator			rCurrRotation;				// Rotation Current 
var vector			vCurrPosition;				// Position Current

var float			fCurrLookAtDistance;		// Current LookAt Distance will change
var float			fMoveBackTightness;			// Current MoveBack tightness (used when camera moves back from hitting a wall or a bBlockCamera actor )

var float			fCurrentMinPitch;			// Current Min pitch the camera can have
var float			fCurrentMaxPitch;			// Current Max pitch the camera can have
var CamSettings		CurrentSet;					// Current Settings the camera is using
var rotator         rBossRotationOffset;        // This is always centered around a zero rotation, then as you move the mouse, it stays within a narrow cone, and is
                                                //  added to rDestRotation.  Gives player ability to 'aim' while in boss cam.
var rotator         rExtraRotation;             // An additional rotation which can be added to the final rotation, say, for camera shake.  Is zero'd every tick.

// --- Settings
var CamSettings		CamSetStandard;				// Settings for the Standard Cam
var CamSettings		CamSetQuidditch;			// Settings for the Quidditch Cam
var CamSettings		CamSetFlyingCar;			// Settings for the FlyingCar Cam
var CamSettings		CamSetCutScene;				// Settings for the CutScene Cam
var CamSettings		CamSetDueling;				// Settings for the Dueling Cam
var CamSettings		CamSetFree;					// Settings for the FreeCam Cam
var CamSettings		CamSetBoss;					// Settings for the FreeCam Cam

var CamSettings		UserSettings[4];			// User defined settings
const NUM_USER_SETTINGS = 4;


// --- Standard Cam specific
var rotator			rSavedRotation;				// saved when you leave standard mode and reloaded when you come back
var vector			vSavedPosition;				// saved when you leave standard mode and reloaded when you come back
var float			fPitchMovingInThreshold;	// set by the constant PITCH_MOVING_IN_THRESHOLD
var float			fPitchMovingInSpread;		// set by the constant PITCH_MOVING_IN_SPREAD
var float			fDistanceScalar;			// obtained by comparing the current pitch with the spread (once you attain the threshold)
var float           fDistanceScalarMin;         // min for fDistanceScalar.  around 0.15.  set by DISTANCE_SCALAR_MIN

// --- Boss Cam specific
var() int           MaxBossAimRot;

// --- Free Cam specific
var ECamMode		LastCamMode;				// last cam mode we were in

// --- CutScene Cam specific
var string			cue;						// send a cut Notify when done

// --- Quiddich Cam specific
//var Snitch			aSnitch;

// --------------------------------------------------------------------------------------------
// *** Constants
const	USE_DEBUG_MODE				= true;

// --- MouseDelta Min and Max constants
const	MIN_MOUSE_DELTA_X			= -20000.0f;	// <- these values will probably change once we get the mouse input correctly
const	MAX_MOUSE_DELTA_X			=  20000.0f;
const	MIN_MOUSE_DELTA_Y			= -10000.0f;
const	MAX_MOUSE_DELTA_Y			=  10000.0f;

// --- Standard Cam Pitch/Moving In constants
const	PITCH_MOVING_IN_THRESHOLD	= 0.0f;			// when pitch reaches this threshold we will start to move the camera in (toward harry)
const	PITCH_MOVING_IN_SPREAD		= 10000.0f;		// spread is the amount needed to attain a scalar of Zero. so if pitch == spread then fDistanceScalar = 0
const   DISTANCE_SCALAR_MIN         = 0.15;



// --------------------------------------------------------------------------------------------
// *** Functions

function float ConvertRotToDeg	( int iRot )		{ return ((float(iRot & 0xFFFF)) / 65536) * 360; }
function float ConvertDegToRot	( float fDeg )		{ return (fDeg / 360) * 65536; }

function SetYaw					( float fYaw	)	{ rDestRotation.Yaw				= fYaw;		}
function SetPitch				( float fPitch	)	{ rDestRotation.Pitch			= fPitch;	}
function SetRoll				( float fRoll	)	{ rDestRotation.Roll			= fRoll;	}							
function SetMinPitch			( float fPitch	)	{ fCurrentMinPitch				= fPitch;	}
function SetMaxPitch			( float fPitch	)	{ fCurrentMaxPitch				= fPitch;	}
function SetRotStep				( rotator step	)	{ rRotationStep					= step;		}
function SetRotStepYaw			( float fYaw	)	{ rRotationStep.yaw				= fYaw;		}
function SetRotStepPitch		( float fPitch	)	{ rRotationStep.pitch			= fPitch;	}
function SetRotStepRoll			( float fRoll	)	{ rRotationStep.roll			= fRoll;	}
function SetRotTightness		( float fTight	)	{ CurrentSet.fRotTightness		= fTight;	}
function SetRotSpeed			( float fSpeed	)	{ CurrentSet.fRotSpeed			= fSpeed;	}
function SetMoveTightness		( float fTight	)	{ CurrentSet.fMoveTightness		= fTight;	}
function SetMoveSpeed			( float fSpeed	)	{ CurrentSet.fMoveSpeed			= fSpeed;	}
function SetDistance			( float fDist	)	{ CurrentSet.fLookAtDistance	= fDist;	}
function SetTargetActor			( name target	)	{ CamTarget.SetAttachedToByName( target );	}
function SetOffset				( vector v		)	{ CamTarget.SetOffset( v );					}
function SetXOffset				( float x		)	{ CamTarget.SetXOffset( x );				}
function SetYOffset				( float y		)	{ CamTarget.SetYOffset( y );				}
function SetZOffset				( float z		)	{ CamTarget.SetZOffset( z );				}
function SetModeByString		( string str	)	{ SetCameraMode( GetModeFromString( str )); }
function SetSyncPosWithTarget	( bool bSyncPos	)	{ bSyncPositionWithTarget = bSyncPos;		}
function SetSyncRotWithTarget	( bool bSyncRot	)	{ bSyncRotationWithTarget = bSyncRot;		}

function SetFOV					( float fFOV	)	
{ 
	playerHarry.DesiredFOV = fFOV;			
	playerHarry.ClientMessage(" playerHarry.DesiredFOV = " $playerHarry.DesiredFOV );
}

function SetPosition( vector pos )
{
//	if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera is setting a new position -> " $pos );

	// calculate our new rotation
	if( bSyncPositionWithTarget )
	{
		// set the correct distance and rotation so that when we are attached
		// we acheive the same location.
		CurrentSet.fLookAtDistance = vsize(CamTarget.location - location);
	
		// make sure our current and Dest rotation is up-to-date
		rDestRotation = rotation;
		rCurrRotation = rotation;
		vDestPosition = location;
		vCurrPosition = location;

		// calculate our new distance and rotation so that we will obtain our Dest Position
		CurrentSet.fLookAtDistance = vsize(CamTarget.location - pos);
		rDestRotation = rotator(normal(  CamTarget.location - pos ));
	}
	else
	{
		vDestPosition = pos;
	}
}



// convert a string into the CamType enum, used for console commands
function ECamMode GetModeFromString( string str )
{
	switch( str )
	{
		case "Startup":		return CM_Startup;
		case "Idle":		return CM_Idle;
		case "Transition":	return CM_Transition;
		
		case "Standard":	return CM_Standard;
		case "FlyingCar":	return CM_FlyingCar;
		case "Quidditch":	return CM_Quidditch;
		case "Dueling":		return CM_Dueling;
		case "CutScene":	return CM_CutScene;
		case "Boss":		return CM_Boss;
		
		case "Free":		return CM_Free;

		default:			return CM_Standard;
	}
}

function SetCameraMode( ECamMode eMode )
{
	switch( eMode )
	{
		case CM_Startup:	LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateStartup');		break;
		case CM_Idle:		LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateIdle');			break;
		
		// If the user SetCameraMode to Transition, assume the user wants to transition to standard mode
		case CM_Transition:	LastCamMode = CameraMode; TransitionToCameraMode( CM_Standard ); break;
		
		case CM_Standard:	LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateStandardCam');	break;
		case CM_FlyingCar:	LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateFlyingCarCam');	break;
		case CM_Quidditch:	LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateQuidditchCam');	break;
		case CM_Dueling:	LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateDuelingCam');		break;
		case CM_CutScene:	LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateCutSceneCam');	break;
		case CM_Boss:		LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateBossCam');		break;

		case CM_Free:		LastCamMode = CameraMode; CameraMode = eMode; GotoState('StateFreeCam');		break;
		
		
		default: log("Camera: Trying to set a camera mode that is not supported!!!");
	}
	
	//DEBUG
	if(USE_DEBUG_MODE)playerHarry.ClientMessage("CameraMode is: " $CameraMode $" with Target:" $CamTarget );
}

function TransitionToCameraMode( ECamMode eMode )
{
	// Set our transition camera mode
	CameraModeTransition = eMode;
	GotoState('StateTransition');
}

// console command helper function that shows the camera's current settings
function ShowSettings()
{
	playerHarry.ClientMessage("The current camera settings are:");
	playerHarry.ClientMessage("-------------------------------------------");	
	playerHarry.ClientMessage("LookAtOffset:        "	$CurrentSet.vLookAtOffset	);
	playerHarry.ClientMessage("LookAtDistance:    "		$CurrentSet.fLookAtDistance	);
	playerHarry.ClientMessage("RotTightness:        "	$CurrentSet.fRotTightness	);
	playerHarry.ClientMessage("RotSpeed:             "	$CurrentSet.fRotSpeed		);
	playerHarry.ClientMessage("MoveTightness:     "		$CurrentSet.fMoveTightness	);
	playerHarry.ClientMessage("MoveSpeed:          "	$CurrentSet.fMoveSpeed		);
	playerHarry.ClientMessage("-------------------------------------------");
	playerHarry.ClientMessage("Current mode:       "	$CameraMode		);
	playerHarry.ClientMessage("Current pos:          "	$vCurrPosition			);
	playerHarry.ClientMessage("Destination pos:      "	$vDestPosition			);
	playerHarry.ClientMessage("Current rot:           "	
		$(rCurrRotation.Yaw)   $" , "
		$(rCurrRotation.Pitch) $" , "
		$(rCurrRotation.Roll)  $" ");
	playerHarry.ClientMessage("Destination rot:      "	
		$(rDestRotation.Yaw)   $" , "
		$(rDestRotation.Pitch) $" , "
		$(rDestRotation.Roll)  $" ");

	playerHarry.ClientMessage("SyncRotationWithTarget: "	$bSyncRotationWithTarget );
	playerHarry.ClientMessage("SyncPositionWithTarget: "	$bSyncPositionWithTarget );
	
	playerHarry.ClientMessage("-------------------------------------------");
	playerHarry.ClientMessage("CamTarget loc:                   " $CamTarget.location );
	playerHarry.ClientMessage("CamTarget rot:                   " $CamTarget.rotation );
	playerHarry.ClientMessage("CamTarget AttachedTo:       " $CamTarget.aAttachedTo );
	playerHarry.ClientMessage("CamTarget Attached loc:     " $CamTarget.aAttachedTo.location );
	playerHarry.ClientMessage("CamTarget attached offset:  " $CamTarget.vOffset );
	playerHarry.ClientMessage("CamTarget relative:            " $CamTarget.bRelative );
}

function LoadUserSettings( int i )
{
	if( i > NUM_USER_SETTINGS-1 )
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("the max user settings index you can have is:" $(NUM_USER_SETTINGS-1) );
		return;
	}
	
	// load settings
	CurrentSet = UserSettings[i];

	if(USE_DEBUG_MODE)playerHarry.ClientMessage("Loaded user settings from slot " $i);
}

function SaveUserSettings( int i )
{
	if( i > NUM_USER_SETTINGS-1 )
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("the max user settings index you can have is:" $(NUM_USER_SETTINGS-1) );
		return;
	}

	// save settings
	UserSettings[i] = CurrentSet;

	if(USE_DEBUG_MODE)playerHarry.ClientMessage("Saved user settings into slot " $i);
}


function PreBeginPlay()
{
	// no collision
	SetCollision(false, false, false);
	bCollideWorld = false;
	
	// Values used for when looking straight up. (we get closer to harry)
	fPitchMovingInThreshold	= PITCH_MOVING_IN_THRESHOLD;
	fPitchMovingInSpread	= PITCH_MOVING_IN_SPREAD;
	fDistanceScalarMin      = DISTANCE_SCALAR_MIN;
}


function PostBeginPlay()
{
	Super.PostBeginPlay();
	
	playerHarry = Harry(Level.playerHarryActor);
	
	
	if( playerHarry == None )
		log("CAMERA CAN NOT FIND HARRY!!!!!!!! in baseCam::PostBeginPlay()" );

	// Create our cut scene target if we havn't already
	if( CamTarget == None )
		CamTarget = spawn( class'BaseCamTarget' );  //CamTarget's owner now is NOT baseCam, it's the other way around.
	                                                // This will help fix the cam update/screen flash problems.  This works in conjunction with my new TickParent member in Actor.uc
	SetOwner( CamTarget );
	CamTarget.Cam = self;  //Camtarget still needs a reference to BaseCam.

	// error checking
	if( CamTarget == None )
	{
		playerHarry.clientmessage("baseCam could not create the hiddenPawn CamTarget!");
		log("CutSceneCam could not create the hiddenPawn CamTarget!");
	}
}

function InitRotation( rotator rot )
{
	rDestRotation.yaw	= rot.yaw	& 0xFFFF;
	rDestRotation.pitch	= rot.pitch & 0xFFFF;
	rDestRotation.roll	= rot.roll  & 0xFFFF;
	vForward			= normal(vector(DesiredRotation));
	rCurrRotation		= rDestRotation;
	DesiredRotation		= rDestRotation;
	SetRotation( DesiredRotation );
	
}

function InitPosition( vector pos )
{
	vDestPosition  = pos;

	// Check our new camera position with the world
	CheckCollisionWithWorld();

	vCurrPosition  = vDestPosition;
	SetLocation( vDestPosition );
}



function InitSettings( CamSettings CamSet, bool bSyncWithTargetPos, bool bSyncWithTargetRot )
{
	// --- Init camera settings
	CurrentSet				= CamSet;
	fDistanceScalar			= 1.0f;
	rRotationStep			= rot(0,0,0);
	rSavedRotation			= rotation;
	bSyncRotationWithTarget	= bSyncWithTargetRot;
	bSyncPositionWithTarget	= bSyncWithTargetPos;
	fDistanceScalarMin      = DISTANCE_SCALAR_MIN;
	fCurrLookAtDistance		= CurrentSet.fLookAtDistance;
}

function InitTarget( actor A )
{
	// -- Init our Camera Target
	CamTarget.SetAttachedTo( A );
	CamTarget.SetOffset( CurrentSet.vLookAtOffset );
}

function InitPositionAndRotation( bool bSnapToNewPosAndRot )
{
	// --- Init our Camera Position and Rotation	
	// If we want to snap to our initial position or have a smooth transition to it
	if( bSnapToNewPosAndRot )
	{
		InitRotation( CamTarget.rotation );
		InitPosition( CamTarget.location+((vec(-CurrentSet.fLookAtDistance,0,0))>>rDestRotation) );
	}
	else
	{
		// For a smooth transition to our Initial Rotation and Position 
		// we should just affect our dest rot and pos.
		
		// Set up our dest rotation
		SetDestRotation( CamTarget.rotation );

		// Set up our dest position
		vDestPosition = CamTarget.location + ((vec(-(CurrentSet.fLookAtDistance),0,0))>>rDestRotation);
		
		// make sure our dest position is correctly placed in the world
		CheckCollisionWithWorld();
	}
	rDestRotation.roll = 0;
	rCurrRotation.roll = 0;
}

function UpdateDistanceScalar( float fTimeDelta )
{
	local float fDestLookAtDistance;
	
	// *** Calculate DistanceScalar, according to what our current pitch is
	if( rCurrRotation.Pitch > fPitchMovingInThreshold )
	{
		// Calculate our DistanceScalar
		fDistanceScalar = 1.0f - ( rCurrRotation.Pitch / fPitchMovingInSpread );
		
		// Cap how close we get to harry's head
		if( fDistanceScalar < fDistanceScalarMin)
			fDistanceScalar = fDistanceScalarMin;
		
		// Calculate our desired lookAt distance based off of our distanceScalar
		fDestLookAtDistance = CurrentSet.fLookAtDistance * fDistanceScalar;
	}
	else
	{
		fDistanceScalar = 1.0f;
		fDestLookAtDistance = CurrentSet.fLookAtDistance;
	}
	
	
	// If our new desired lookAt distance is less than what we currently have then set curr = desired.
//	if( fDesiredLookAtDistance < fCurrLookAtDistance )
//		fCurrLookAtDistance = fDesiredLookAtDistance;
	
	// If the camera needs to start falling back from hitting a wall or a bBlockCamera actor. 
	// ( determined in CheckCollisionWithWorld )
	if(	fCurrLookAtDistance < fDestLookAtDistance )
	{
		// smoothly have our distance from our LookAt point increase over time
		fCurrLookAtDistance += (fDestLookAtDistance - fCurrLookAtDistance ) * FMin( 1.0f, fMoveBackTightness * fTimeDelta );		 	
	}
	else
	{
		fCurrLookAtDistance = fDestLookAtDistance;
	}
	
} // end UpdateDistanceScalar


// 
// UpdateRotationUsingVectors is not an optimal solution for solving the new DestRotation problem.
//
// Problem being that if the dest rotation is set rather than having a += by a rot delta then you can
// run into problems where rot.yaw = 0 at one tick then rot.yaw = 65534 in another. 
// (These rotations are right next to each other ) but because the formula is (Dest - Curr) the camera
// will spin around 360 degrees.
//
// What you should do is use a function that detects these changes and compensates by adding or subtracting.
// 
function UpdateRotationUsingVectors( float fTimeDelta )
{
	local float		fTravelScalar;
	local vector	vDestRotation, vCurrRotation;

	vDestRotation = normal(vector(rDestRotation));
	vCurrRotation = vForward;

	// *** Update Rotation
	// If we are attached to our target then
	if( bSyncRotationWithTarget )
	{
		//playerHarry.ClientMessage( "CamTarget = " $CamTarget.location $" rot = " $ rotation );
		
		// our current rotation will always face CamTarget
		vDestRotation = CamTarget.location - location;
		vCurrRotation = vDestRotation;
	}
	else
	{
		// --- Compute and clamp our travel scalar (time is involved)
		if( CurrentSet.fRotTightness > 0.0f )
			fTravelScalar = FMin( 1.0f, CurrentSet.fRotTightness * fTimeDelta );
		else
			fTravelScalar = 1.0f;
		
		// Smoothly update rotation
		vCurrRotation += ( vDestRotation - vCurrRotation ) * fTravelScalar;	
	}
	
	
	vCurrRotation = normal(vCurrRotation);
	rCurrRotation = rotator(vCurrRotation);

	// Update our current forward vector
	vForward = vCurrRotation;
	
	// Finally set our rotation
	SetFinalRotation( rotator(vCurrRotation) );
	
	//DEBUG
//	playerHarry.ClientMessage("cur Rot = " $rCurrRotation $" target Rot = " $vDestRotation );
} // end UpdateRotation()



function ApplyMouseXToDestYaw( float fTimeDelta, optional bool bApplyToBossOffset )
{
	// *** Update our mouse vars
	fMouseDeltaX = playerHarry.SmoothMouseX * fTimeDelta;
	
	// --- CAP our MouseDelta X
	if( fMouseDeltaX > MAX_MOUSE_DELTA_X )		fMouseDeltaX = MAX_MOUSE_DELTA_X;
	else if( fMouseDeltaX < MIN_MOUSE_DELTA_X ) fMouseDeltaX = MIN_MOUSE_DELTA_X;
	
	// *** Update Dest Rotation	
	if( !bApplyToBossOffset )
	{
		rDestRotation.Yaw   += fMouseDeltaX * CurrentSet.fRotSpeed;
	}
	else
	{
		rBossRotationOffset.Yaw   += fMouseDeltaX * CurrentSet.fRotSpeed;
		if( rBossRotationOffset.Yaw >  MaxBossAimRot )
			rBossRotationOffset.Yaw =  MaxBossAimRot;
		else
		if( rBossRotationOffset.Yaw < -MaxBossAimRot )
			rBossRotationOffset.Yaw = -MaxBossAimRot;
	}
}

function ApplyMouseYToDestPitch( float fTimeDelta, optional bool bApplyToBossOffset )
{
	// *** Update our mouse vars
	fMouseDeltaY = playerHarry.SmoothMouseY * fTimeDelta;	
	
	// Support bInvertMouse
	if( playerHarry.bInvertMouse )
		fMouseDeltaY = -fMouseDeltaY;

	// --- CAP our MouseDelta Y	
	if( fMouseDeltaY > MAX_MOUSE_DELTA_Y )		fMouseDeltaY = MAX_MOUSE_DELTA_Y;
	else if( fMouseDeltaY < MIN_MOUSE_DELTA_Y ) fMouseDeltaY = MIN_MOUSE_DELTA_Y;
	
	// *** Update Dest Rotation	
	if( !bApplyToBossOffset )
	{
		rDestRotation.Pitch += fMouseDeltaY * CurrentSet.fRotSpeed;
		
		// Cap the Dest Pitch
		if( rDestRotation.Pitch > fCurrentMaxPitch )		rDestRotation.Pitch  = fCurrentMaxPitch;
		else if( rDestRotation.Pitch < fCurrentMinPitch )	rDestRotation.Pitch  = fCurrentMinPitch;
	}
	else
	{
		rBossRotationOffset.Pitch += fMouseDeltaY * CurrentSet.fRotSpeed;
		if( rBossRotationOffset.Pitch >  MaxBossAimRot )
			rBossRotationOffset.Pitch =  MaxBossAimRot;
		else
		if( rBossRotationOffset.Pitch < -MaxBossAimRot )
			rBossRotationOffset.Pitch = -MaxBossAimRot;
	}
}

function SetDestRotation( rotator newRot )
{
	rDestRotation = newRot;
}


function UpdateRotation( float fTimeDelta )
{
	local float	fTravelScalar;
	
	// *** Update Rotation
	
	// --- Apply a rotation step over time ( if there is any )
	rDestRotation += rRotationStep * fTimeDelta;
	
	// If bSyncWithTarget == true, immediatly face target
	if( bSyncRotationWithTarget )
	{
		rCurrRotation = rotator(CamTarget.location - location);
	}
	else // Smoothly Face Target over time
	{
		// --- Compute and clamp our travel scalar (time is involved)
		if( CurrentSet.fRotTightness > 0.0f )
			fTravelScalar = FMin( 1.0f, CurrentSet.fRotTightness * fTimeDelta );
		else
			fTravelScalar = 1.0f;
		
		// Smoothly update rotation
		rCurrRotation += ( rDestRotation - rCurrRotation ) * fTravelScalar;	
	}
	
	// Update our current forward vector
	vForward = normal(vector(rCurrRotation));
	
	// Finally set our rotation
	SetFinalRotation( rCurrRotation );
}


function SetFinalRotation( rotator r )
{
	r += rExtraRotation;
	rExtraRotation = rot(0,0,0);

	DesiredRotation = r;
	SetRotation( r );
}


function UpdatePosition( float fTimeDelta )
{
	local float fTravelScalar;

	// *** Update Position
	
	// If we want our destination to sync with the target 
	if( bSyncPositionWithTarget )
	{
		// compute our target position
		vDestPosition = CamTarget.location + 
			((vec(-(fCurrLookAtDistance ),0,0)) >> rCurrRotation );	
	}
	
	// Check our new camera position with the world
	CheckCollisionWithWorld();
	
/*
	// If the camera needs to start falling back from hitting a wall or a bBlockCamera actor. 
	// ( determined in CheckCollisionWithWorld )
	if(	fCurrLookAtDistance < CurrentSet.fLookAtDistance )
	{
		// smoothly have our distance from our LookAt point increase over time
		fCurrLookAtDistance += (CurrentSet.fLookAtDistance - fCurrLookAtDistance ) * FMin( 1.0f, fMoveBackTightness * fTimeDelta );		 	
	}
*/	
	
	// we are already at our target so we don't need to update
//	if( vDestPosition == vCurrPosition )
//		return;
	
	// --- Compute and clamp our travel scalar
	// NOTE: the travel scalar is how much we will travel the distance between our current pos and target pos.
	// Since we should never go farther than our target we need to make sure its capped at 1.0f
	if( CurrentSet.fMoveTightness > 0.0f )
		fTravelScalar = FMin( 1.0f, CurrentSet.fMoveTightness * fTimeDelta );
	else
		fTravelScalar = 1.0f;

	// --- Smoothly update position
	vCurrPosition += ( vDestPosition - vCurrPosition ) * fTravelScalar;
	SetLocation( vCurrPosition );
}



function bool CheckCollisionWithWorld()
{
	local vector	HitLocation;
	local vector	HitNormal;
	local actor		HitActor;
	local vector	LookAtPoint;
	local vector    LookFromPoint;
	local vector	vCusionFromWorld;

	// Use a local var for CamTarget's location because it may be altered by a collision check
	LookAtPoint		= CamTarget.location;
	
	// *** FIRST do a FastTrace with the line from the target actor's Location to the CamTarget's location, 
	// if there is an offset.
	if( CamTarget.aAttachedTo != None && 
		(CamTarget.vOffset.x != 0 || CamTarget.vOffset.y != 0 || CamTarget.vOffset.z != 0) )
	{
		// Our CamTarget has an offset, so we need to make sure that it isn't outside of the world.
		// If our CamTarget.location is outside of the world our TraceActors function will fail, to prevent this we will
		// make sure that we use a CamTarget.location that is inside of the world.
		
		HitActor = Trace( HitLocation, HitNormal, CamTarget.Location, CamTarget.aAttachedTo.location, false );
		if( HitActor != None && HitActor.IsA('levelInfo') )
		{
			// The CamTarget has hit the world!!!
			LookAtPoint = HitLocation + ( normal(CamTarget.aAttachedTo.location - HitLocation) * 5.0f) + HitNormal;
			playerHarry.ClientMessage("CamTarget HitLoc:" $HitLocation $" HitNorm: " $HitNormal );
		}
	}
	
	// The Cusion from world will make sure our camera isn't exactly on the same plane as the geometry we hit
	vCusionFromWorld = normal(LookAtPoint-vDestPosition) * 5.0f;
	LookFromPoint    = vDestPosition - vCusionFromWorld;
	
	// *** SECOND Trace the line segment between our camera position and the LookAtPoint With ACTORS
	foreach TraceActors(class'actor', HitActor, HitLocation, HitNormal, LookFromPoint, LookAtPoint )
	{
		// Trivially reject actors we don't care about
		if( HitActor == Owner )
			continue;
		
		// See if we care about this actor
		if( HitActor.IsA('levelInfo') || HitActor.bBlockCamera )
		{
			// We hit something that obstructs the camera
			// Make our new target position be where we hit the world plus a small offset toward our look at position
			vDestPosition       = HitLocation + vCusionFromWorld;// + HitNormal;
			fCurrLookAtDistance = vsize(vDestPosition - LookAtPoint);

			//DEBUG
			//playerHarry.ClientMessage("Hit " $HitActor $" new destposition" $vDestPosition );
			return true;
		}
	}
	
	return false;
}


// --------------------------------------------------------------------------------------------
// *** States

auto state StateStartup
{
	function BeginState()
	{
	}

	begin:
	
	// Init our camera
	InitSettings( CamSetStandard, true, false );
	InitTarget( playerHarry );
	InitPositionAndRotation( true );
	
	// Set our inital camera mode
	SetCameraMode( CM_Standard );
}

state StateIdle
{
	// don't do anything
}

//******************************************************************************************************
//************************
//**** TRANSITION CAM ****
//************************
// this state is used when transitioning from one mode to another... smoothly
state StateTransition
{
	function BeginState()
	{		
		// Set up our target so that it transitions to the correct position
		CamTarget.SetAttachedTo( None );
		CamTarget.DoFlyTo( playerHarry.location + CamSetStandard.vLookAtOffset, MOVE_TYPE_EASE_TO, 1.0);
		
		// Init the camera as a standard camera
		InitSettings( CamSetStandard, false, true );
		InitRotation( playerHarry.rotation );
		
		CurrentSet.fMoveTightness	= 0.1f;
		
		// Set up our dest position
		vDestPosition = playerHarry.location + CamSetStandard.vLookAtOffset +
						( (vec(-CurrentSet.fLookAtDistance,0,0))>>rDestRotation );
		
		playerHarry.ClientMessage(" 1 DestRot = " $rDestRotation $" CurRot = " $rCurrRotation );
	}
	
	function Tick( float fTimeDelta )
	{
		// As time goes on, go faster toward harry
		CurrentSet.fMoveTightness += fTimeDelta * 4.0f;
		
		// *** Update Camera
		// Set up our dest rotation and position
		
		// --- Update our Rotation
		UpdateRotation( fTimeDelta );
		
		// --- Update our Position
		UpdatePosition( fTimeDelta );
		
		// --- See if we have reached our destination
		
		// Test for our destination threshold
		if( vsize(vCurrPosition - vDestPosition) <= 0.01f )
		{	
			playerHarry.ClientMessage(
				" Transition DestRot = " $rDestRotation 
				$" CurRot = " $rCurrRotation 
				$"TargetLoc = " $CamTarget.location );
			
			CamTarget.SetAttachedTo( playerHarry );
			CamTarget.SetOffset( CamSetStandard.vLookAtOffset );
			CutCue( sCutNotifyCue );// Send cue
			SetCameraMode( CameraModeTransition );
		}
	}
}

//******************************************************************************************************
//**********************
//**** STANDARD CAM ****
//**********************

state StateStandardCam
{
	ignores takeDamage, SeePlayer, EnemyNotVisible, HearNoise, KilledBy, Trigger, Bump, HitWall, HeadZoneChange, FootZoneChange, ZoneChange, Falling, WarnTarget, Died, LongFall, PainTimer;

	function BeginState()
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> StandardCam");
		
		// Init our camera
		InitSettings( CamSetStandard, true, false );
		InitTarget( playerHarry );
		InitPositionAndRotation( true );
	}
	
	
	function EndState()
	{
		// save our current rotation
		rSavedRotation = rCurrRotation;
	}

	function Tick( float fTimeDelta )
	{
		// *** Update Camera	
		// --- Apply Mouse Input to our Dest rotation
		ApplyMouseXToDestYaw( fTimeDelta );
		ApplyMouseYToDestPitch( fTimeDelta );

		// --- Update our Rotation
		UpdateRotation( fTimeDelta );
		
		// --- Update our Position
		UpdatePosition( fTimeDelta );
			
		// --- Update our Distance scalar
		UpdateDistanceScalar( fTimeDelta );

		// ***TODO: going to do this diffrently so there isn't a check every update
		// right now there is a nasty hack in the \Engine\Src\UnGame.cpp (line 1954) that sets the bInSpecialPause.
		if( bInSpecialPause )
		{
			SetCameraMode( CM_Free );
		}
	}

	begin:
} // end StateStandardCam


//******************************************************************************************************
//***********************
//**** Quidditch CAM ****
//***********************
state StateQuidditchCam
{
	function BeginState()
	{

		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> QuidditchCam");
		
		// Init our camera
		InitSettings( CamSetQuidditch, true, false );
		InitTarget( playerHarry );
		InitPositionAndRotation( true );

		// get our snitch actor
//		foreach AllActors( class'Snitch', aSnitch )
//			break;
	}

	function Tick( float fTimeDelta )
	{
		local vector	LookDir;
		local rotator	rSavedCurrRotation;

		// *** Update Camera

		// Temporarily point camera directly at camera target in order to set position
		rSavedCurrRotation = rCurrRotation;
		rCurrRotation = rotator( CamTarget.Location - vCurrPosition );
		UpdatePosition( fTimeDelta );
		rCurrRotation = rSavedCurrRotation;

		// Now point camera half way between camera target actor and Harry just to look at them
		LookDir = 0.5 * (PlayerHarry.Location - vCurrPosition)
				+ 0.5 * (CamTarget.Location - vCurrPosition);
		rDestRotation = rotator( LookDir );

		// --- Update our Rotation
		UpdateRotationUsingVectors( fTimeDelta );
		
		// ***TODO: going to do this diffrently so there isn't a check every update
		// right now there is a nasty hack in the \Engine\Src\UnGame.cpp (line 1954) that sets the bInSpecialPause.
		if( bInSpecialPause )
		{
			SetCameraMode( CM_Free );
		}
	}
}


//******************************************************************************************************
//***********************
//**** FlyingCar CAM ****
//***********************
state StateFlyingCarCam
{
	function BeginState()
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> FlyingCarCam");

		// Init our camera
		InitSettings( CamSetFlyingCar, true, false );
		InitTarget( playerHarry );
		InitPositionAndRotation( true );
	}

	function Tick( float fTimeDelta )
	{
		local rotator rot;

		// *** Update Camera
		rot.yaw   = playerHarry.rotation.yaw & 0xFFFF;
		rot.pitch = playerHarry.rotation.pitch & 0xFFFF;
		SetDestRotation( rot );
				
		// --- Update our Rotation
		UpdateRotationUsingVectors( fTimeDelta );
		
		// --- Update our Position
		UpdatePosition( fTimeDelta );
		
		// ***TODO: going to do this diffrently so there isn't a check every update
		// right now there is a nasty hack in the \Engine\Src\UnGame.cpp (line 1954) that sets the bInSpecialPause.
		if( bInSpecialPause )
		{
			SetCameraMode( CM_Free );
		}
	}
}



//******************************************************************************************************
//*********************
//**** DUELING CAM ****
//*********************

state StateDuelingCam
{
	ignores takeDamage, SeePlayer, EnemyNotVisible, HearNoise, KilledBy, Trigger, Bump, HitWall, HeadZoneChange, FootZoneChange, ZoneChange, Falling, WarnTarget, Died, LongFall, PainTimer;

	function BeginState()
	{
		local rotator rot;

		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> StandardCam");
		
		// Init our camera
		InitSettings( CamSetDueling, true, false );
		InitTarget( playerHarry );
		InitPositionAndRotation( true );
	}

	function Tick( float fTimeDelta )
	{
		// *** Update Camera	
		// --- Apply Mouse Input to our Dest rotation
//		ApplyMouseYToDestPitch( fTimeDelta );
		
		// --- Update our Rotation
		UpdateRotation( fTimeDelta );
		
		// --- Update our Position
		UpdatePosition( fTimeDelta );

		// --- Update our Distance scalar
		UpdateDistanceScalar( fTimeDelta );
			
		// ***TODO: going to do this diffrently so there isn't a check every update
		// right now there is a nasty hack in the \Engine\Src\UnGame.cpp (line 1954) that sets the bInSpecialPause.
		if( bInSpecialPause )
		{
			SetCameraMode( CM_Free );
		}
	}

	begin:
} // end StateStandardCam


//******************************************************************************************************
//***********************
//**** CUT-SCENE CAM ****
//***********************

state StateCutSceneCam
{
	function BeginState()
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> StateCutSceneCam");

		// Reset our distance scalar
		fDistanceScalar		= 1.0f;
		rRotationStep		= rot(0,0,0);
		
		// Update our camTarget's location before we go into our freeCam , (so there is no pop)
		CamTarget.SetLocation( CamTarget.aAttachedTo.location + CamTarget.vOffset );
		CamTarget.aAttachedTo = None;
		
		// Set our camera's current settings to the cut scene cam settings
		CurrentSet		= CamSetCutScene;
		CamTarget.SetOffset( CurrentSet.vLookAtOffset );
		
		rDestRotation.roll	= 0;
		rCurrRotation.roll	= 0;
		
		// the CutSceneCam defaults the "bSyncPositionWithTarget" to false
		bSyncPositionWithTarget = false;
		bSyncRotationWithTarget = true; // always face our target when doing a flyto

	}

	function Tick( float fTimeDelta )
	{	
		super.Tick( fTimeDelta );
				
		// *** Update Camera

		// --- Update Rotation
		UpdateRotation( fTimeDelta );
		
		// --- Update Position (if attached)
		if( bSyncPositionWithTarget )
			UpdatePosition( fTimeDelta );

		// bInSpecialPause check (to put the camera in free mode or not)
		if( bInSpecialPause )
			SetCameraMode( CM_Free );
	}
}


//******************************************************************************************************
//******************
//**** Boss CAM ****
//******************
state StateBossCam
{
	function BeginState()
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> BossCam");

		// Init our camera
		InitSettings( CamSetBoss, true, false );
		InitTarget( playerHarry );
		InitPositionAndRotation( false );
	}

	function Tick( float fTimeDelta )
	{		
		local vector v;

		ApplyMouseXToDestYaw( fTimeDelta, true/*bApplyToBossOffset*/ );
		ApplyMouseYToDestPitch( fTimeDelta, true/*bApplyToBossOffset*/ );
		
		// *** Update Camera
		if( baseBoss(playerHarry.BossTarget) != none )
			v = baseBoss(playerHarry.BossTarget).GetCamTargetLoc();
		else
			v = playerHarry.BossTarget.Location;

		//rDestRotation = rotator(normal( v - CamTarget.location ));
		rDestRotation = rotator(normal( v - /*CamTarget.*/location ));
		//Add in the boss offset
		rDestRotation += rBossRotationOffset;
		
		// --- Update our Rotation
		UpdateRotationUsingVectors( fTimeDelta );
		
		// --- Update our Position
		UpdatePosition( fTimeDelta );
		
		// ***TODO: going to do this diffrently so there isn't a check every update
		// right now there is a nasty hack in the \Engine\Src\UnGame.cpp (line 1954) that sets the bInSpecialPause.
		if( bInSpecialPause )
			SetCameraMode( CM_Free );
	}
}

//******************************************************************************************************
//******************
//**** FREE CAM ****
//******************
state StateFreeCam
{
	ignores takeDamage, SeePlayer, EnemyNotVisible, HearNoise, KilledBy, Trigger, Bump, HitWall, HeadZoneChange, FootZoneChange, ZoneChange, Falling, WarnTarget, Died, LongFall, PainTimer;
	
	
	function BeginState()
	{
		if(USE_DEBUG_MODE)playerHarry.ClientMessage("Camera: BeginState -> FreeCam");
		
		// --- Init camera settings
		CurrentSet			= CamSetFree;
		fDistanceScalar		= 1.0f;
		rRotationStep		= rot(0,0,0);
		rSavedRotation		= rotation;
		bSyncPositionWithTarget	= false;
		bSyncRotationWithTarget	= false;
	}
	
		
	function Tick( float fTimeDelta )
	{
		// *** Check "specialPause" mode
		if( !bInSpecialPause )
		{
			// If we are not in "SpecialPause mode then 
			// warp harry to this position (if our target.. is harry)
			if( CamTarget.aAttachedTo.IsA('Harry') )
				Harry(CamTarget.aAttachedTo).GotoLocation( location );
			
			// then goto standard cam
			SetCameraMode( LastCamMode );
		}

		// *** Update our mouse vars
		fMouseDeltaX = playerHarry.SmoothMouseX * fTimeDelta;
		fMouseDeltaY = playerHarry.SmoothMouseY * fTimeDelta;
		
		// *** CAP our MouseDelta X and Y
		if( fMouseDeltaX > MAX_MOUSE_DELTA_X )		fMouseDeltaX = MAX_MOUSE_DELTA_X;
		else if( fMouseDeltaX < MIN_MOUSE_DELTA_X ) fMouseDeltaX = MIN_MOUSE_DELTA_X;
		
		if( fMouseDeltaY > MAX_MOUSE_DELTA_Y )		fMouseDeltaY = MAX_MOUSE_DELTA_Y;
		else if( fMouseDeltaY < MIN_MOUSE_DELTA_Y ) fMouseDeltaY = MIN_MOUSE_DELTA_Y;

		// !!! The code that gets Key Input was copied out of the old freeCam !!!		
		// *** Update Position

		// Add to our positin by our key input
		if( baseconsole(playerHarry.player.console).bForwardKeyDown )
			vDestPosition += (vect(1, 0, 0) >> Rotation) * CurrentSet.fMoveSpeed  * fTimeDelta;
		else if( baseconsole(playerHarry.player.console).bBackKeyDown )
			vDestPosition += (vect(-1, 0, 0) >> Rotation) * CurrentSet.fMoveSpeed * fTimeDelta;
		
		if( baseconsole(playerHarry.player.console).bRightKeyDown )
			vDestPosition += (vect(0, 1, 0) >> Rotation) * CurrentSet.fMoveSpeed  * fTimeDelta;
		else if( baseconsole(playerHarry.player.console).bLeftKeyDown )
			vDestPosition += (vect(0, -1, 0) >> Rotation) * CurrentSet.fMoveSpeed * fTimeDelta;
		
		if( baseconsole(playerHarry.player.console).bUpKeyDown )
			vDestPosition += (vect(0, 0, 1) >> Rotation) * CurrentSet.fMoveSpeed  * fTimeDelta;
		else if( baseconsole(playerHarry.player.console).bDownKeyDown )
			vDestPosition += (vect(0, 0, -1) >> Rotation) * CurrentSet.fMoveSpeed * fTimeDelta;
		
		// --- Smoothly update position
		vCurrPosition += ( vDestPosition - vCurrPosition ) * FMin( 1.0f, CurrentSet.fMoveTightness * fTimeDelta );
		SetLocation( vCurrPosition );		

		// *** Update rotation
		// Add our rotation from key input
		if( baseconsole(playerHarry.player.console).bRotateRightKeyDown)
			rDestRotation.Yaw += CurrentSet.fRotSpeed * fTimeDelta;
		else if( baseconsole(playerHarry.player.console).bRotateLeftKeyDown)
			rDestRotation.Yaw -= CurrentSet.fRotSpeed * fTimeDelta;
		
		if( baseconsole(playerHarry.player.console).bRotateUpKeyDown)
			rDestRotation.Pitch += CurrentSet.fRotSpeed * fTimeDelta;
		else if( baseconsole(playerHarry.player.console).bRotateDownKeyDown)
			rDestRotation.Pitch -= CurrentSet.fRotSpeed * fTimeDelta;
		
		// Add rotation from mouse input
		rDestRotation.Yaw   += fMouseDeltaX * CurrentSet.fRotSpeed;
		rDestRotation.Pitch += fMouseDeltaY * CurrentSet.fRotSpeed;
		
		// --- Smoothly update rotation
		rCurrRotation += (rDestRotation - rCurrRotation ) * FMin( 1.0f, CurrentSet.fRotTightness * fTimeDelta );
		DesiredRotation = rCurrRotation;
		SetRotation( DesiredRotation );
	}
	
begin:
}




//***********************************************************************************************************
//Called from BaseCamTarget when things like "FlyTo" are done.
//function BaseCamTargetEvent( name EventName )
//{
//	playerHarry.ClientMessage("BaseCamTargetEvent event");
//
//	if( EventName == 'ActionDone' )
//	{
//		if( CutNotifyActor != None  &&  sTargetCutNotifyCue != "" )
//			CutNotifyActor.CutCue( sTargetCutNotifyCue );
//
//		sTargetCutNotifyCue = "";
//	}
//}

//***********************************************************************************************************
//command is like "MoveTo Target Arg1 Arg2 ....."
//cue is the cue to send when the action is complete. Save this somewhere for when the command is finished.
//bFastFlag is for the fastforward stuff. Its not hooked up to anything yet tho.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string	sActualCommand;
	local string	sString;
	local int		i;
	local bool      b;

	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	if( sActualCommand ~= "Capture" )
	{
		playerHarry.ClientMessage("*** Camera Captured");
		SetCameraMode( CM_CutScene );
		return true;
	}
	else
	if( sActualCommand ~= "Release" )
	{
		playerHarry.ClientMessage("*** Camera Released");
		CamTarget.CutCommand( "Release", "", false );

		SetCameraMode( CM_Standard );
	
		// Flow through to base class
	}
	else
	if( sActualCommand ~= "GoHome" )
	{
		log("*** GoHome CALLED!!!!  loc = " $location $" rot = " $rotation );		
		
		bSyncPositionWithTarget = true;
		SetPosition(location);

		// Set the camera to transition toward harry
		SetCameraMode( CM_Transition );
		
		// We need to save the cue for later
		sCutNotifyCue = cue;
		return true;
	}
	else
	if( sActualCommand ~= "FlyTo" )
	{
		bSyncPositionWithTarget = false;
		
		if( !bIgnoreTarget )
			bSyncRotationWithTarget = true;		// face our target when doing a flyto ( unless we are ignoring it)
		
		//Flow through to super case...
	}
	else 
	if( sActualCommand ~= "Target" )
	{	
		return CutCommand_ProcessTarget(command, cue, bFastFlag);
	}
	else
	if( sActualCommand ~= "IgnoreTargetOn" )
	{
		bIgnoreTarget = true;
		bSyncPositionWithTarget = false;	// don't move with the target
		bSyncRotationWithTarget = false;	// don't rotate with the target
		sCutNotifyCue = cue;
		DoCutCueNotify();
		return true;		
	}
	else
	if( sActualCommand ~= "IgnoreTargetOff" )
	{
		bIgnoreTarget = false;
		sCutNotifyCue = cue;
		DoCutCueNotify();
		return true;		
	}
	else
	if( sActualCommand ~= "Locked" )
	{
		bSyncPositionWithTarget = true;		// because we are locked we need to sync our position with the target
		bSyncRotationWithTarget = true;		// smoothly face the same direction as our target
		
		return CutCommand_ProcessLocked(command, cue, bFastFlag);
	}
	else
	if( sActualCommand ~= "UnLock" )
	{
		bSyncPositionWithTarget = false;	// when unlocked don't move with the target
		bSyncRotationWithTarget = true;		// always face our target when unlocked
		sCutNotifyCue = cue;
		DoCutCueNotify();
		return true;
	}
	else
	if( sActualCommand ~= "FOV" )
	{
		return CutCommand_ProcessFOV(command, cue, bFastFlag);
	}
	else
	if( sActualCommand ~= "Shake" )
	{
		return CutCommand_ProcessShake( command, cue, bFastFlag);
	}
	else
	if( sActualCommand ~= "Flash" )
	{
		return CutCommand_ProcessFlash( command, cue, bFastFlag);
	}
	else
	if( sActualCommand ~= "FadeOut" )
	{
		return CutCommand_ProcessFade( true, command, cue, bFastFlag);
	}
	else
	if( sActualCommand ~= "FadeIn" )
	{
		return CutCommand_ProcessFade( false, command, cue, bFastFlag);
	}
	
	return  super.CutCommand(command, cue, bFastFlag);
}

function bool CutCommand_ProcessFOV( string command, optional string cue, optional bool bFastFlag )
{
	local FOVController Controller;
	local TimedCue		tcue;
	local string		sString;
	local float			fAngle, fTime;
	local int			i;
	
	// Set up our defaults
	fAngle		= 90.0f;
	fTime		= 0.25f;
	
	// all other camera keywords are unique to camera settings
	for( i = 2; i < 8; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		if( Left(sString, 6) ~= "Angle=" )
			fAngle = float(Mid(sString,6));
		else
		if( Left(sString, 5) ~= "Time=" )
			fTime = float(Mid(sString,5));
		if( sString == "" )
			break;
	}//end for
	
	
	if( bFastFlag )
		fTime = 0.0f;
	
	// Spawn and Setup our FOVController
	Controller = spawn( class'FOVController' );
	Controller.Init( fAngle, fTime );

	//create a TimedCue to cue object after sndLen seconds.
	tcue = spawn(class 'TimedCue');
	tcue.CutNotifyActor=self;			//Tell me when done. This is auto passed back to the CutNotifyActor if any.
										//Or it can be used by the talk to find out when the talk is finished.
	tcue.SetupTimer(fTime+0.5,cue);		//little extra time for slop
	return true;
}


function bool CutCommand_ProcessFlash( string command, optional string cue, optional bool bFastFlag )
{
	local FadeViewController FadeController;
	local TimedCue	tcue;
	local string	sString;
	local bool		bUseDefault;
	local float		A,R,G,B,fTime;
	local int		i;
	
	// Set up our defaults
	A			= 255;
	bUseDefault = true;
	fTime		= 0.25f;
	
	// all other camera keywords are unique to camera settings
	for( i = 2; i < 8; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		if( Left(sString, 2) ~= "A=" )
			{ A = float(Mid(sString,2)); bUseDefault = false; }
		else
		if( Left(sString, 2) ~= "R=" )
			{ R = float(Mid(sString,2)); bUseDefault = false; }
		else
		if( Left(sString, 2) ~= "G=" )
			{ G = float(Mid(sString,2)); bUseDefault = false; }
		else
		if( Left(sString, 2) ~= "B=" )
			{ B = float(Mid(sString,2)); bUseDefault = false; }
		else
		if( Left(sString, 5) ~= "Time=" )
			fTime = float(Mid(sString,5));
		else
		if( sString == "" )
			break;
	}//end for
	
	// Convert 0-255 into 0-1
	A = FClamp(A/255, 0.0f, 1.0f);
	R = FClamp(R/255, 0.0f, 1.0f);
	G = FClamp(G/255, 0.0f, 1.0f);
	B = FClamp(B/255, 0.0f, 1.0f);

	FadeController = spawn(class'FadeViewController');
	
	if( bUseDefault )
	{ R = 1.0f; G = 1.0f; B = 1.0f; }

	if( bFastFlag )
	{
		FadeController.Init(A,R,G,B, 0.0f, true );
		CutCue( cue );
		return true;
	}
	
	FadeController.Init(A,R,G,B, fTime, true );

	//create a TimedCue to cue object after sndLen seconds.
	tcue = spawn(class 'TimedCue');
	tcue.CutNotifyActor=self;			//Tell me when done. This is auto passed back to the CutNotifyActor if any.
										//Or it can be used by the talk to find out when the talk is finished.
	tcue.SetupTimer(fTime+0.5,cue);		//little extra time for slop
	return true;
}

//************************************************************************************************************************

function bool CutCommand_ProcessShake(string command, optional string cue, optional bool bFastFlag )
{
	local TimedCue	tcue;
	local string	sString;
	local float		fMagnitude, fTime;
	local int		i;
	
	fMagnitude = 100.0f;
	fTime	   = 0.5f;
	
	// all other camera keywords are unique to camera settings
	for( i = 2; i < 8; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		if( Left(sString, 10) ~= "Magnitude=" )
			fMagnitude = float(Mid(sString,10));
		else
		if( Left(sString, 5) ~= "Time=" )
			fTime = float(Mid(sString,5));
		else
		if( sString == "" )
			break;
	}//end for

	if( bFastFlag )
	{
		CutCue( cue );
	}
	else // we want a cut cue
	{
		//create a TimedCue to cue object after sndLen seconds.
		tcue = spawn(class 'TimedCue');
		tcue.CutNotifyActor=self;			//Tell me when done. This is auto passed back to the CutNotifyActor if any.
											//Or it can be used by the talk to find out when the talk is finished.
		tcue.SetupTimer(fTime+0.5,cue);		//little extra time for slop	
	}

	//Camera shake...
	playerHarry.ShakeView( fTime, fMagnitude, fMagnitude );

	return true;
}


//************************************************************************************************************************


function bool CutCommand_ProcessFade( bool bFadeOut, string command, optional string cue, optional bool bFastFlag )
{
	local FadeViewController FadeController;
	local TimedCue	tcue;
	local string	sString;
	local float		A,R,G,B,fTime;
	local int		i;
	
	if( bFadeOut )
		A = 255;

	fTime = 1.0f;

	// all other camera keywords are unique to camera settings
	for( i = 2; i < 8; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		if( Left(sString, 2) ~= "A=" && bFadeOut )
			A = float(Mid(sString,2));
		else
		if( Left(sString, 2) ~= "R=" && bFadeOut )
			R = float(Mid(sString,2));
		else
		if( Left(sString, 2) ~= "G=" && bFadeOut )
			G = float(Mid(sString,2));
		else
		if( Left(sString, 2) ~= "B=" && bFadeOut )
			B = float(Mid(sString,2));
		else
		if( Left(sString, 5) ~= "Time=" )
			fTime = float(Mid(sString,5));
		else
		if( sString == "" )
			break;
	}//end for
	
	// Convert 0-255 into 0-1
	A = FClamp(A/255, 0.0f, 1.0f);
	R = FClamp(R/255, 0.0f, 1.0f);
	G = FClamp(G/255, 0.0f, 1.0f);
	B = FClamp(B/255, 0.0f, 1.0f);
	
	FadeController = spawn(class'FadeViewController');
	
	if( bFastFlag )
	{
		FadeController.Init(A,R,G,B, 0.0f, false );
		CutCue( cue );
		return true;
	}

	FadeController.Init(A,R,G,B, fTime, false );

	//create a TimedCue to cue object after sndLen seconds.
	tcue = spawn(class 'TimedCue');
	tcue.CutNotifyActor=self;			//Tell me when done. This is auto passed back to the CutNotifyActor if any.
										//Or it can be used by the talk to find out when the talk is finished.
	tcue.SetupTimer(fTime+0.5,cue);		//little extra time for slop
	return true;
}


//************************************************************************************************************************
function bool CutCommand_ProcessLocked(string command, optional string cue, optional bool bFastFlag)
{
	local string	sString;
	local int		i;
	local bool      b;
	

	// set the correct distance and rotation so that when we are attached
	// we acheive the same location.
	CurrentSet.fLookAtDistance = vsize(CamTarget.location - location);
	
	// make sure our current and target rotation is up-to-date
	rDestRotation = rotation;
	rCurrRotation = rotation;	
	vDestPosition = location;
	vCurrPosition = location;
	
	// all other camera keywords are unique to camera settings
	for( i = 2; i < 15; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );
		
		//DEBUG
		//playerHarry.ClientMessage(" locked sString -> " $sString );
		
		if( Left(sString, 9) ~= "distance=" )
			SetDistance( float(Mid(sString,9)) );
		else
		if( Left(sString, 4) ~= "yaw=" )
			SetYaw(  ConvertDegToRot( float(Mid(sString,4))) );
		else
		if( Left(sString, 6) ~= "pitch=" )
			SetPitch(ConvertDegToRot( float(Mid(sString,6))) );
		else
		if( Left(sString, 5) ~= "roll=" )
			SetRoll( ConvertDegToRot( float(Mid(sString,5))) );
		else
		if( Left(sString, 8) ~= "yawStep=" )
		{	rRotationStep.yaw = ConvertDegToRot( float(Mid(sString,8))); bSyncRotationWithTarget = false; }
		else
		if( Left(sString, 10) ~= "pitchStep=" )
		{	rRotationStep.pitch = ConvertDegToRot( float(Mid(sString,10))); bSyncRotationWithTarget = false; }
		else
		if( Left(sString, 9) ~= "rollStep=" )
		{	rRotationStep.roll = ConvertDegToRot( float(Mid(sString,9))); bSyncRotationWithTarget = false; }
		else
		if( Left(sString, 13) ~= "rotTightness=" )
			SetRotTightness( float(Mid(sString,13)) );
		else
		if( Left(sString, 14) ~= "moveTightness=" )
			SetMoveTightness( float(Mid(sString,14)) );
		else
		if( sString == "" )
			break;

	}//end for
	
	// we just did a settings change so save our cue and send a cutCueNotify
	sCutNotifyCue = cue;
	DoCutCueNotify();
	return true;
}

//************************************************************************************************************************
function bool CutCommand_ProcessTarget(string command, optional string cue, optional bool bFastFlag)
{
	local string	sActualCommand;
	local string	sString;
	local int		i;
	local bool      b;
	local bool      bPassToTarget;

	bPassToTarget = true;

	sString = ParseDelimitedString( command, " ", 2, false );

	if( sString ~= "flyto" )
	{	
		// make sure our target is not attached to anything
		CamTarget.aAttachedTo = none;
		//flow through to super case
	}
	else if( sString ~= "teleport" )
	{
		// make sure our target is not attached to anything
		CamTarget.aAttachedTo = none;
		//flow through to super case
	}
	else
	{
		for( i = 2; i < 20; i++ )
		{
			sString = ParseDelimitedString( command, " ", i, false );
		
			//DEBUG
			//playerHarry.ClientMessage(" target sString -> " $sString );
				
			// Added functionality for the baseCam target
			if( Left(sString, 11) ~= "attachedto=" )
			{
				bPassToTarget = false;

				//DEBUG
				//playerHarry.ClientMessage("!!!!!!!!!!!! SetAttachedTo Called with input -> " $Mid(sString,11) );
				if( !CamTarget.SetAttachedToByCutName( Mid(sString,11) ) )
				{
					playerHarry.ClientMessage("!*!*!* COULD NOT ATTACH TARGET TO: " $Mid(sString,11) );
					return false;
				}
			}
			else
			if( Left(sString, 2) ~= "x=" )
			{
				CamTarget.vOffset.x = float( Mid(sString,2) );
				SetZOffset( CamTarget.vOffset.x );

				bPassToTarget = false;
			}
			else
			if( Left(sString, 2) ~= "y=" )
			{
				CamTarget.vOffset.y = float( Mid(sString,2) );
				SetZOffset( CamTarget.vOffset.y );

				bPassToTarget = false;
			}
			else
			if( Left(sString, 2) ~= "z=" )
			{
				//DEBUG
				//playerHarry.ClientMessage("z= Called with input -> " $Mid(sString,2) );
				CamTarget.vOffset.z = float( Mid(sString,2) );
				SetZOffset( CamTarget.vOffset.z );
				bPassToTarget = false;
			}
			else
			if( sString ~= "relative" )
			{		
				//DEBUG
				//playerHarry.ClientMessage("relative Called");
				CamTarget.bRelative = true;
				bPassToTarget = false;
			}
			else
			if( sString ~= "fixed" )
			{
				CamTarget.bRelative = false;
				bPassToTarget = false;
			}
			else if( sString == "" )
				break;
		}//end for
	}
	
	
	if( bPassToTarget )
	{
		// DEBUG
//		playerHarry.ClientMessage("Passing target string -> " $ParseDelimitedString( command, " ", 2, true ) );
	
		CamTarget.CutNotifyActor = CutNotifyActor;
		
		b = CamTarget.CutCommand( ParseDelimitedString( command, " ", 2, true ), cue, bFastFlag );
		if( !b )
			CutErrorString = CamTarget.CutErrorString;
		return b;
	}
	
	// we just did a settings change so save our cue and send a cutCueNotify
	sCutNotifyCue = cue;
	DoCutCueNotify();
	return true;
}

function bool CameraCanSeeYou(vector pos)
{
	local vector normal;
	local float dotpr;

	normal = vector(Rotation);

	// checking if you are on the same side of plane, where rotator points
	dotpr = normal.X * (pos.X - Location.X) + normal.Y * (pos.Y - Location.Y) + normal.Z * (pos.Z - Location.Z);
	if(dotpr > 0)
		return true;

	return false;
}

// --------------------------------------------------------------------------------------------
// *** Default Properties

defaultproperties
{
	// --- Default Cam Settings
	CameraMode=CM_Startup
	bSyncPositionWithTarget=true

	fCurrentMinPitch=-14000.0f	
	fCurrentMaxPitch=14000.0f

	fDistanceScalar=1.0f
	
	fMoveBackTightness=2.5f

	// --- Standard Cam Settings
	CamSetStandard=(vLookAtOffset=(X=0,Y=0,Z=55.0f))
	CamSetStandard=(fLookAtDistance=128.0f)
	CamSetStandard=(fRotTightness=8.0f)
	CamSetStandard=(fRotSpeed=4.0f)
	CamSetStandard=(fMoveTightness=0.0f)
	CamSetStandard=(fMoveSpeed=0.0f)
	
	// --- Quidditch Cam Settings
	CamSetQuidditch=(vLookAtOffset=(X=0,Y=0,Z=65.0f))
	CamSetQuidditch=(fLookAtDistance=65.0f)
	CamSetQuidditch=(fRotTightness=2.0f)
	CamSetQuidditch=(fRotSpeed=0.0f)
	CamSetQuidditch=(fMoveTightness=7.0f)
	CamSetQuidditch=(fMoveSpeed=0.0f)
	
	// --- FlyingCar Cam Settings
	CamSetFlyingCar=(vLookAtOffset=(X=0,Y=0,Z=175.0f))
	CamSetFlyingCar=(fLookAtDistance=400.0f)
	CamSetFlyingCar=(fRotTightness=4.0f)
	CamSetFlyingCar=(fRotSpeed=0.0f)
	CamSetFlyingCar=(fMoveTightness=4.0f)
	CamSetFlyingCar=(fMoveSpeed=0.0f)

	// --- CutScene Cam Settings
	CamSetCutScene=(vLookAtOffset=(X=0,Y=0,Z=0))
	CamSetCutScene=(fLookAtDistance=128.0f)
	CamSetCutScene=(fRotTightness=2.0f)
	CamSetCutScene=(fRotSpeed=5.0f)
	CamSetCutScene=(fMoveTightness=0.0f)
	CamSetCutScene=(fMoveSpeed=0.0f)
	
	// --- Dueling Cam Settings
	CamSetDueling=(vLookAtOffset=(X=0,Y=0,Z=45.0f))
	CamSetDueling=(fLookAtDistance=200.0f)
	CamSetDueling=(fRotTightness=8.0f)
	CamSetDueling=(fRotSpeed=4.0f)
	CamSetDueling=(fMoveTightness=3.5f)
	CamSetDueling=(fMoveSpeed=0.0f)

	// --- Free Cam Settings
	CamSetFree=(vLookAtOffset=(X=0,Y=0,Z=0.0f))
	CamSetFree=(fLookAtDistance=0.0f)
	CamSetFree=(fRotTightness=10.0f)
	CamSetFree=(fRotSpeed=5.0f)
	CamSetFree=(fMoveTightness=7.0f)
	CamSetFree=(fMoveSpeed=600.0f)

	// --- Boss Cam Settings
	CamSetBoss=(vLookAtOffset=(X=0,Y=0,Z=100.0f))
	CamSetBoss=(fLookAtDistance=170.0f)
	CamSetBoss=(fRotTightness=8.0f)
	CamSetBoss=(fRotSpeed=4.0f)
	CamSetBoss=(fMoveTightness=0.0f)
	CamSetBoss=(fMoveSpeed=0.0f)
	MaxBossAimRot=4000
	
	// --- HPawn Settings
	bRotateToDesired=false
	
	bHidden=true
	bBlockActors=false
	bBlockPlayers=false

 	bCanMoveInSpecialPause=true

	CutName="baseCam"

	bIgnoreZonePainDamage=true
}

// --------------------------------------------------------------------------------------------
// baseCam.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------