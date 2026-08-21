
// Class Name  : FlyingFordHedwig
//
// Created on  : 06/15/2002
// Authored by : Janet Weddle
// 
// Description : Hedwig flies in front of the flying ford but stays on the path that the 
//				 guide is one. 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class FlyingFordHedwig extends HiddenHpawn;

var bool		bTouch;			// Has this object been touched

var Director	Director;		// Current object in control of the mini-game or puzzle that Harry's playing



// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{
	// Find mini-game director
	foreach AllActors( class'Director', Director )
		break;

}

function touch (actor other)
{
	
	// Hedwig should not receive a touch. 
	if ( bTouch == false )
	{
		bTouch = true;
	
		Director.OnTouchEvent( Self, Other );
	}


}

function untouch(actor other)
{
	bTouch = false;
	
	Director.OnUnTouchEvent( Self, Other );


}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

defaultproperties
{
	 Tag='FlyingFordHedwig'
	 Mesh=SkeletalMesh'HPModels.skowlbarnMesh'
	 DrawType=DT_Mesh
	 DrawScale=1.2
     CollisionRadius=35
     CollisionHeight=32
     bCollideActors=False
     bCollideWorld=False
	 bTouch=False
	 bHidden=False

	 bTrailerSameRotation=True
	 bTrailerPrePivot=True
}
