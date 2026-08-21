
// Class Name  : FlyingFordLightning
//
// Created on  : 06/14/2002
// Authored by : Janet Weddle
// 
// Description : This marks the area where the lightning is striking.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class FlyingFordLightning extends HiddenHpawn;

var bool		bTouch;			// Has this object been touched

var FlyingFordDirector	Director;		// Current object in control of the mini-game or puzzle that Harry's playing
var() name		stormName;		// The lightning zone and the lightning all have the same name
var() float		fLightningViolence;	// The violence of the lightning strike
var() int		iLightningLoops;	// The number of times the car changes direction
var() float		fTimeBetweenchanges; // The amount of time between each direciton change



// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{
	// Find mini-game director
	foreach AllActors( class'FlyingFordDirector', Director )
		break;

}

function touch (actor other)
{
	if ( other.IsA('Harry') )
	{
		// The car has entered a lightning zone
		if ( bTouch == false )
		{
			bTouch = true;

//			playerHarry.clientMessage("Have received a touch should be calling StartLightning");
	
			Director.OnTouchEvent( Self, Other );
//			Director.StartLightning(fLightningViolence,self);
		}
	}


}

function untouch(actor other)
{

	Super.UnTouch(other);

	// You have left this lightning zone
//	bTouch = false;

	if ( other.IsA('Harry') )
	{
//		playerHarry.clientMessage("UNTouch from :  " $other);
		Director.OnUnTouchEvent( Self, Other );
	}


}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

defaultproperties
{
	 Tag='FlyingFordLightning'
	 drawType=DT_SPRITE
     CollisionRadius=35
     CollisionHeight=32
     bCollideActors=True
     bCollideWorld=True
	 bTouch=False
	 bHidden=True

	 fLightningViolence=5
	 iLightningLoops=15
	 fTimeBetweenchanges=0.2
}
