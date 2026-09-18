

// Class Name  : FlyingFordTown
//
// Created on  : 05/24/2002
// Authored by : Janet Weddle
// 
// Description : This marks a town zone in the flying ford anglia mini-game. If the flying ford
//				 is in this area the muggle-meter will go up quickly
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class FlyingFordTown extends HiddenHpawn;

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
	

	// The car will be in these alot. Only send the one touch message
	// Since these 'zones' will be overlapped Director should probably keep a refcount
	if ( bTouch == false )
	{
		bTouch = true;
	
		Director.OnTouchEvent( Self, Other );
	}


}

function untouch(actor other)
{
	// You have left this safe zone
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
	 drawType=DT_SPRITE
	 Tag='FlyingFordTown'
     CollisionRadius=500
     CollisionHeight=32
     bCollideActors=True
     bCollideWorld=True
	 bTouch=False
	 bCollideWhenPlacing=True  // In theory this should check for a collision when it spawns. 
}
