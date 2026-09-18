
// Class Name  : PixieDust
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : PixieDust AI. The dust dropped from a pixie as it flies around. 
//				 at this point none will be dropped as the pixie attacks Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class PixieDust extends HiddenHpawn;

var bool bTouch;
var float fLifetime;
//var bool bCanBeThrown;
var float timeSafe;
var bool bCanBeTouched;
var() int sleepyInterval;


// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{

	setTimer(fLifetime,false);
	bCollideWorld = true;
}

function timer()
{
	Destroy();
}	

function touch (actor other)
{

	if ( other == playerHarry && bCanBeTouched == true )
	{
		bCanBeTouched = false;

		if ( baseHud( playerharry.myHud ).bCutSceneMode == false)
		{
			// This will be sleepy
			playerHarry.SleepyAnimTimerAdd(sleepyInterval);
//			CornishPixie(owner).PixieDustHit();

		}
	}

}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

defaultproperties
{
	 drawType=DT_NONE
     attachedParticleClass(0)=Class'HPParticle.PixieGroundDust'
	 Physics=PHYS_FALLING
	 Mass=10
	 sleepyInterval=2
     CollisionRadius=25
     CollisionHeight=32
     bCollideActors=True
     bCollideWorld=True
	 bTouch=True
	 bCanBeTouched=true
	 fLifetime=5.0
}



