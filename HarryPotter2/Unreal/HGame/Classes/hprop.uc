//=============================================================================
// HProp
//=============================================================================
class HProp expands HPawn;

const START_CAM_ZOOM_DIST = 160.0;

// Pickup properties
enum EPickupFlyTo
{
	FT_None,        
	FT_Camera,		
	FT_HudPosition,	
	FT_DropOffInWorld,
};

var sound        soundPickup;
var sound        soundPickup2;        // If want to randomize between 2 pickup sounds
var sound        soundDropOff;
var bool         bPickupOnTouch;      // True to pickup when touched by Harry
var name         EventToSendOnPickup; // normally none
var int          nPickupIncrement;    // Increment count by this much on Pickup
var EPickupFlyTo PickupFlyTo;         // If bPickupOnTouch==true, prop will fly
                                      // away in this manner

									  // Used when PickupFlyTo==FT_HudPosition
var float        fTotalFlyTime;		  // Total fly time 
var float        fCurrFlyTime;        // Current fly time
var vector       vectHudLocation;     // Location of corresponding hud item 
                                      // in 3D space

                                      // Used when PickupFlyTo==FT_Camera;
var float        fCurrCameraZoomDist; // Current dist to camera
var bool         bReadyForFlyEffect;

var vector       vectDropOffLoc;
var bool         bDestroyAfterDropOff;

var float        fMinFlyToHudScale;     // Minimum we'll scale down the prop when flying to hud
var float        fMaxFlyToHudScale;     // When prop flies to hud, start draw scale will be this

// Status manager props.  Link the prop to a status group and status item.
var (StatusManager) class<StatusGroup> classStatusGroup;
var (StatusManager) class<StatusItem>  classStatusItem; 

// Bounce into place vars.  To get bouncing when a derived prop is spawned:
//	1) Set bBounceIntoPlace to true in derived class
//  2) Set soundBounce to the sound to play when bouncing.
//  3) Make an empty "auto state BounceIntoPlace" state
var bool	bBounceIntoPlaceTiming;
var float	fBounceIntoPlaceTimeout;
var bool    bBounceIntoPlace;
var sound   soundBounce;

function PreBeginPlay()
{
	super.PreBeginPlay();

	if (bBounceIntoPlace)
	{
		bBounce = true;
		SetPhysics(PHYS_Falling);
		bBounceIntoPlaceTiming = false;
	}
}

// Actor touched the prop
event touch (actor other)
{
	if(bPickupOnTouch==true &&		       // If want to pickup on touch
	   other==playerharry &&               // Harry's the one who touched us
	   GetStateName()!='PickupProp' &&     // We're not already in the pickup state
       !playerHarry.IsEngagedWithVendor()&& // not in vendor mode
       !playerHarry.IsMixingPotion())       // not in potion mixing mode
	{
		DoPickupProp();		
	}
}


// Rotate prop to face camera.
function FaceCamera()
{
	local Rotator r;

	r = playerharry.cam.Rotation;   
	r.yaw += 65536/4;                 // ??? These numbers were from HP1 
	r.roll = r.pitch;
	r.pitch = 0;
	DesiredRotation = r;
	SetRotation(r);
}

// Locate prop at the current zoom to camera distance.
function ZoomToCamera()
{
	local vector v;

	v.x = fCurrCameraZoomDist;
	SetLocation(playerharry.cam.location+(v >> playerharry.cam.Rotation));
}

// Move the prop to the current position on the fly to hud path.
function FlyToNewPosition(float fMovePercent)
{
	local vector vectNewLoc;    
	local bool   bMovedSmooth;

	// New position is the final location minus the current location
	// divided by the percent we want to move this time.
	if (PickupFlyTo == FT_HudPosition)
		vectNewLoc  = (vectHudLocation - location) / fMovePercent;
	else if (PickupFlyTo == FT_DropOffInWorld)
		vectNewLoc  = (vectDropOffLoc - location) / fMovePercent;
	else
		playerHarry.ClientMessage("ERROR:  Unrecognized PickupFlyTo value");
	
	MoveSmooth(vectNewLoc);

	// As the object gets closer to the hud, shrink it down.
	DrawScale *= 0.95;
	if (DrawScale < fMinFlyToHudScale)
		DrawScale = fMinFlyToHudScale;
}

// RenderHud will only get called for the prop after it's been registered
// (in either PickupProp state or DropOffProp state).
function RenderHud(Canvas canvas)
{
	canvas.DrawActor(self, false, true);
}

function DoPickupProp()
{
	gotostate('PickupProp');            
}

// Take prop from the hud and place it at a location in the world.  This
// function assumes that the prop is at the proper starting location
// (probably at the hud item location). 
function DoDropOffProp(vector vSetDropOff, bool bDestroyWhenDone)
{
	vectDropOffLoc       = vSetDropOff;
	bDestroyAfterDropOff = bDestroyWhenDone;
	gotostate('DropOffProp');	
}

event HitWall(vector HitNormal, actor Wall)
{
	playerHarry.ClientMessage("hitwall " $wall.name);

	if (IsInState('PickupProp') || IsInState('DropOffProp'))
	{
		bHidden = true;
		bCollideWorld = false;
		SetCollision(false, false, false);
		HPHud(playerHarry.myHud).RegisterPickupProp(self);
	}
}

/*
event Bump(actor Other)
{
	playerHarry.ClientMessage("bumped " $other.name);

}
*/

function SetFlyProps()
{
	SetPhysics(PHYS_Flying);
	bBounce = false;

	// If we're going to be flying this prop, we don't want it
	// colliding with anything.
	//bCollideWorld = false;
	bCollideWorld = true;
	SetCollision(false, false, false);
}


function TickPickupOrDropOff(float delta)
{
	local vector dest;

	// If not ok to start the effect, return and try again later.
	if (!bReadyForFlyEffect)
		return;

	switch (PickupFlyTo)
	{
	case (FT_HudPosition):
	case (FT_DropOffInWorld):
		fCurrFlyTime -= delta;

		if (fCurrFlyTime > 0)
			FlyToNewPosition(fCurrFlyTime/delta);

		break;
	case (FT_Camera):
		fCurrCameraZoomDist -= delta * 400;
		if (fCurrCameraZoomDist > 0)
		{
			ZoomToCamera();
			FaceCamera();
		}
		break;
	default:
		playerHarry.ClientMessage("ERROR:  Pickup type not recognized");
		break;
	}
}

state DropOffProp
{
	ignores touch;

	// Over time, move the prop to it's new position (if it's supposed to
	// fly somewhere on pickup.
	event tick(float delta)
	{
		TickPickupOrDropOff(delta);
	}

	event BeginState()
	{
		PickupFlyTo = FT_DropOffInWorld;

		// Set a flag to make sure all necessary "begin" label stuff gets setup
		// before tick code starts moving the prop to the hud.
		bReadyForFlyEffect = false;
	}

begin:
    DrawScale = fMaxFlyToHudScale;

	// Play sound for dropping.
	if (soundDropOff != None)
		PlaySound(soundDropOff);

	SetFlyProps();
		
	// Set current fly time to the total time the prop should fly.
	fCurrFlyTime = fTotalFlyTime;

	// Let the StatusManager know that we've been pickedup.
	playerHarry.managerStatus.DropOffItem(self);

	// While the prop is still flying, keep giving time back to the system.
	bReadyForFlyEffect = true;
	while(fCurrFlyTime>0)
		sleep(0.1);

	if (bDestroyAfterDropOff == true)
	{
		// Destroy oursleves.
		bHidden = true;
		HPHud(playerHarry.myHud).UnregisterPickupProp(self);
		destroy();
	}
}

//-----------------------------------------------------------------------------------
//  State PickupProp
//-----------------------------------------------------------------------------------
//
//  State while prop is being picked up (if prop can be picked up).

state PickupProp
{
	// Don't detect touch anymore.  We don't want Harry to touch the prop while
	// it is in the process of flying away or being destroyed.
	ignores touch;

	// Over time, move the prop to it's new position (if it's supposed to
	// fly somewhere on pickup.
	event tick(float delta)
	{
		TickPickupOrDropOff(delta);
	}

	event BeginState()
	{
		// Set a flag to make sure all necessary "begin" label stuff gets setup
		// before tick code starts moving the prop to the hud.
		bReadyForFlyEffect = false;
	}

begin:

    DrawScale = fMaxFlyToHudScale;

	// Play sound for picking up.  If have 2, randomly pick one of them.
    // If just have one, play that.
	if (soundPickup != None && soundPickup2 != None)
    {
        if (Rand(2) == 0)
		    PlaySound(soundPickup);
        else
            PlaySound(soundPickup2);
    }
    else if (soundPickup != None)
        PlaySound(soundPickup);
    else if (soundPickup2 != None)
        PlaySound(soundPickup2);

	SetFlyProps();
		
	// If prop is flying to it's corresponding StatusItem on the hud.
	if (PickupFlyTo == FT_HudPosition)
	{
		// Set current fly time to the total time the prop should fly.
		fCurrFlyTime = fTotalFlyTime;

		// Get the location of the hud in 3D space.  Calculate just once
		// here and use each time we need find the new fly prop position.
		vectHudLocation = playerHarry.managerStatus.GetHudLocation(self);

		// While the prop is still flying, keep giving time back to the system
		bReadyForFlyEffect = true;
		while(fCurrFlyTime>0)
			sleep(0.1);
	}

	// If prop is supposed to fly into the camera.
	else if (PickupFlyTo == FT_Camera)
	{
		// Face the prop to the camera
		FaceCamera();

		// Set distance away from camera.
		fCurrCameraZoomDist = START_CAM_ZOOM_DIST;

		// While prop is still flying, keep giving time back to the system.
		bReadyForFlyEffect = true;
		while (fCurrCameraZoomDist > 0)
			sleep(0.2);
	}

	// Let the StatusManager know that we've been picked up.
	playerHarry.managerStatus_PickupItem(self);

	//See if we're supposed to trigger an event on pickup
	if( EventToSendOnPickup != '' )
		TriggerEvent( EventToSendOnPickup, self, self );

	// Destroy oursleves.
	bHidden = true;
	HPHud(playerHarry.myHud).UnregisterPickupProp(self);
	destroy();
}

state BounceIntoPlace
{
	function BeginState()
	{
		bBounceIntoPlaceTiming = false;
		fBounceIntoPlaceTimeout = 5.0;
	}

	function tick(float deltatime)
	{
		local Rotator	NewRotation;

		Super.tick(deltatime);

		NewRotation = Rotation;
		NewRotation.Yaw += (30000 * deltatime);
		NewRotation.Yaw = NewRotation.Yaw & 0xffff;

		SetRotation(NewRotation);

		if (bBounceIntoPlaceTiming)
		{
			fBounceIntoPlaceTimeout -= deltatime;
		}
	}

	function HitWall( vector HitNormal, actor Wall )
	{
		Velocity *= 0.5;
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );

		bBounceIntoPlaceTiming = true;
		if (bBounceIntoPlaceTiming && fBounceIntoPlaceTimeout >= 0)
		{
			if (abs(Velocity.z) > 10 && soundBounce != None)
				playsound(soundBounce, , abs(Velocity.z) / 100, , , );
		}
	}

	begin:
	loop:
		sleep(1);
		goto 'loop';
}

defaultproperties
{
     AmbientGlow=75
     bBlockActors=True
     bBlockPlayers=True
	 bBlockCamera=True

	 bJumpOffPawn=false

	 // Pickup related.  By default, prop cannot be picked up.
	 //  This means pickup and put in your inventory/hud
	 // NOT pickup and carry in your hand visually.
	 bPickupOnTouch=false
	 nPickupIncrement=1
	 PickupFlyTo=FT_None
	 fTotalFlyTime=.25
	 fCurrFlyTime=0
	 soundPickup=None
	 soundDropOff=None
     fMinFlyToHudScale=0.5
     fMaxFlyToHudScale=1.0
	 
	 bGestureFaceHorizOnly=false
}
