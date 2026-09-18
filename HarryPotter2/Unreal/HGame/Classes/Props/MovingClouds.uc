
// Class Name  : MovingClouds
//
// Created on  : 05/24/2002
// Authored by : Janet Weddle
// 
// Description : These clouds are set in front of the Flying Ford Anglia and give a more 
// realistic and 3D look
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class MovingClouds extends HiddenHpawn;

var bool		bTouch;			// Has this object been touched
var() float		fFrontOffset;	// How far in front of the car will it be placed
var() float		fSideOffset;	// How far to the side of the car will it be placed (0 is in the middle)
var() bool		bFollowCar;		// Will this object stay in the same place or move with the car (at an offset)
var() bool		bFollowPath;	// Will stay a certain distance from the car but will offset from the path
var vector		vCloudLocation;	// The location of the clouds offset from the car
var vector		vDir;			// The rotation direction of the moving cloud

var Director	Director;		// Current object in control of the mini-game or puzzle that Harry's playing



// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{
	// Find mini-game director
	foreach AllActors( class'Director', Director )
		break;

	vCloudLocation = FindLocation();
	SetLocation(vCloudLocation);

}

function touch (actor other)
{	
	Director.OnTouchEvent( Self, Other );
}

function untouch(actor other)
{

	Director.OnUnTouchEvent( Self, Other );
}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

// Check if the car has moved past the clouds (the ones that don't move with the car)
function bool PassedClouds()
{
	local vector	vCarLocation;
	local vector	vStraightDir, vCarDir;

	vStraightDir = -(vector(rotation));
	vCarDir = vector(playerHarry.rotation);

	if ( (vStraightDir dot vCarDir) < 0 )
	{
		clientMessage("We have passed the clouds");
		return true;
	}

	return false;

}


function vector FindLocation()
{
	local rotator direction;
	local vector vOffset;
	local vector vLocation;

	direction = playerHarry.rotation;
	direction.Yaw = direction.Yaw + fSideOffset;
	
	vOffset = vector(direction) * fFrontOffset;

	vLocation = playerHarry.location + vOffset;

	return vLocation;

}

function vector FindLocationFromPath()
{
	local rotator direction;
	local vector vOffset;
	local vector vLocation;

	local PatrolPoint PPClosestToCar, PPClosestToCarNext;
	local PatrolPoint PPClosestToLine, PPClosestToLineNext;
	local rotator lineRotation, destRotation;
	local vector fLine;
	local float  fDistanceFromPoint;

	// Find the closest patrol point and the next one
	PPClosestToCar = FindNearestPatrolPoint(playerHarry.location);
	PPClosestToCarNext = PPClosestToCar.NextPatrolPoint;

	// Get the rotation at that point
	lineRotation = rotator(PPClosestToCarNext.location - PPClosestToCar.location);

	// Throw out a line along that rotation to a point fFrontOffset from car
	fLine = playerHarry.location + (vector(lineRotation) * fFrontOffset);

	// Find the patrol point closest to the line and the next one
	PPClosestToLine = FindNearestPatrolPoint(fLine);

	// Find the distance from the patrol point to where you're going to place the cloud (the line)
	fDistanceFromPoint = vSize2D(PPClosestToLine.location-fLine);

	if ( PPClosestToLine.NextPatrolPoint != None )
	{
		PPClosestToLineNext = PPClosestToLine.NextPatrolPoint;
		destRotation = rotator(PPClosestToLineNext.location - PPClosestToLine.location);
	}
	else
	{
		destRotation = PPClosestToLine.rotation;
	}

	// Set the rotation.yaw to include fSideOffset
	destRotation.Yaw = destRotation.Yaw + fSideOffset;

	// Set the end offset to be at the new rotation the correct distance from the patrol point
	vOffset = vector(destRotation) * fDistanceFromPoint;

	// Add the offset to the patrol points location
	vLocation = PPClosestToLine.location + vOffset;

	return vLocation;

}

function PatrolPoint FindNearestPatrolPoint(vector loc)
{
	local PatrolPoint   tempPatrolPoint, closestPoint;
	local float		    fDist, fClosestDist;
	local vector		vLocation;

	vLocation = loc;

	fClosestDist = 1000000;

	// Check each point and find the one closest to the car
	foreach AllActors(class'PatrolPoint', tempPatrolPoint)
	{
		fDist = VSize( vLocation - tempPatrolPoint.Location );
	
		if( fDist < fClosestDist )
		{
			fClosestDist = fDist;
			closestPoint = tempPatrolPoint;
		}
	}

	return closestPoint;
}


function Tick(float DeltaTime)
{
	local vector vDirection;
	local rotator vRot;
	// can put in a delta so that it's a little behind all of the time. tweak.

	// If both are true than bFollowCar will overwrite (since it's usually closer a mistake should be more obvious)
	if ( bFollowPath == true )
	{
		vCloudLocation = FindLocationFromPath();
	}

	if ( bFollowCar == true )
	{
		vCloudLocation = FindLocation();
	}

	SetLocation(vCloudLocation);

//	vDirection = ( playerHarry.location - location );
//	vRot = rotator(vDirection);
//	vRot.pitch += 16384;
//	setRotation(vRot);
//	DesiredRotation = (vRot);

		
//	DesiredRotation.Yaw = 65535 - playerHarry.rotation.Yaw;
//	DesiredRotation.Pitch = 65535 - playerHarry.rotation.Pitch;
//	DesiredRotation.Roll = 65535 - playerHarry.rotation.Roll;

	// change the rotation of the object in the level. to reflect the beginning
	// rotation of the car. Leave it that way. 




}

defaultproperties
{
	 Tag='MovingClouds'
	 attachedParticleClass(0)=Class'HPParticle.FlyingClouds'
	 attachedParticleClass(1)=None
	 drawType=DT_SPRITE
	 drawScale=5
     CollisionRadius=35
     CollisionHeight=32
     bCollideActors=False
     bCollideWorld=False
	 bTouch=False
	 bHidden=False

	 fFrontOffset=100
	 fSideOffset=8191	// Yes, this is odd but I want to give the level designers a place to start
	 bFollowCar=True
}
