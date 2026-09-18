
// Class Name  : FlyingFordWindTrigger
//
// Created on  : 06/11/2002
// Authored by : Janet Weddle
// 
// Description : The actor that will trigger the wind particles to begin
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class FlyingFordWindTrigger extends HiddenHpawn;

var bool		bTouch;			// Has this object been touched
 
var FlyingFordDirector	Director;		// Current object in control of the mini-game or puzzle that Harry's playing



// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{
	Super.PostBeginPlay();

	// Find mini-game director
	foreach AllActors( class'FlyingFordDirector', Director )
		break;

}

function touch (actor other)
{

	local FlyingFordWind wind;

	Super.Touch(other);

//	playerHarry.clientMessage("Touched wind trigger : "  $ tag);
//	log("Wind Trigger has been touched");
	// When the car touches a wind zone trigger. Send a touch.
	if ( other.IsA('Harry') )
	{
		if ( bTouch == false )
		{
			bTouch = true;
	
			Director.OnTouchEvent( Self, Other );
			wind = FlyingFordWind(owner);
			if ( wind != None )
			{
//				playerHarry.clientMessage("Starting wind now");
				wind.StartWind();
			}

		}

	}


}

function untouch(actor other)
{
	local FlyingFordWind wind;

	Super.UnTouch(other);

	if ( other.IsA('Harry') )
	{
		// You have left the wind trigger
		bTouch = false;
		
		Director.OnUnTouchEvent( Self, Other );
		wind = FlyingFordWind(owner);
		if ( wind != None )
			wind.StopWind();
	}


}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}


auto state triggerBegin
{
begin:
//log("What is my position and radius  : " $location$ "  " $collisionRadius);

}

defaultproperties
{
	 Tag='FlyingFordWindTrigger'
	 CollideType=CT_Cylinder
	 drawType=DT_SPRITE
	 CollisionHeight=400
     CollisionRadius=0
     CollisionWidth=0
     bCollideActors=True
     bCollideWorld=True
	 bTouch=False
	 bHidden=False
}
