

// Class Name  : FlyingFordPathGuide
//
// Created on  : 05/24/2002
// Authored by : Janet Weddle
// 
// Description : This is the hidden pawn that follows the spline in the flying ford level
//				 All car movement will be offset from this. 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class FlyingFordPathGuide extends HiddenHpawn;


var Director	Director;		// Current object in control of the mini-game or puzzle that Harry's playing

var name	pathName;
var float   AirSpeedNormal;

// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{

	// Find mini-game director
	foreach AllActors( class'Director', Director )
		break;

	SetCollision(false,false,false);

}

function touch (actor other)
{
	Super.Touch(other);
	// I am not sending a touch event to the director since it doesn't matter if I touch or not
}

function untouch(actor other)
{
	Super.UnTouch(other);
	// I am not sending a untouch event to the director since it doesn't matter if I touch or not
}


function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

auto state startFlying
{
begin:
 
	FollowSplinePath( pathName,				//optional name  PathTagName
					AirSpeedNormal,		//optional float speed
					0 ,						//optional float accel
					,						//optional name  StartPointName
					 						//optional name  EndPointName
					);

}
 
defaultproperties
{
//	 drawType=DT_SPRITE
	 drawType=DT_MESH
	 Mesh=SkeletalMesh'HPModels.skfirecrabMesh'
	 Tag='FlyingFordPathGuide'
     CollisionRadius=200
     CollisionHeight=50
     bCollideActors=False
     bCollideWorld=False
	 

	 bHidden=True
}
