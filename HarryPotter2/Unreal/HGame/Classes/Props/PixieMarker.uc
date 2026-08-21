
// Class Name  : PixieMarker
//
// Created on  : 07/03/2002
// Authored by : Janet Weddle
// 
// Description : PixieMarker marks spots where pixies will stay. They will effectively 'cage'
//				 the pixies to an area. Pixie will still need splines to follow
//
// NOTE:	   : 07/03/2002
//				 PixieMarker is basically the same as SpiderMarker
//
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class PixieMarker extends Actor;

#exec Texture Import File=Textures\Hidpawn.pcx Name=HiddenPawn Mips=Off Flags=2

// *** Variables

var() name groupName; // The pixie will stay in the area with the same name


// *** Constants
 
const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

function PreBeginPlay()
{
	super.PreBeginPlay();

	//Is this needed?   YES!! It is!  Just putting the four defaults down in defaultproperties WONT make the actor
	// move normally through the world with SetLocation().  You have to call SetCollision to make the actor be TRULY non colliding.
	SetCollision(,,);
	bCollideWorld = false;

}

function postBeginPlay()
{
	SetCollision( true, false, false );

}

function touch (actor other)
{
	
	// This doesn't really need a touch. It's only for a location reference
	Super.Touch(other);

}

function untouch(actor other)
{
	Super.UnTouch(other);

}

function bump( actor other)
{
	touch(other);
}

defaultproperties
{
	Texture=Texture'HGame.HiddenPawn'
    DrawType=DT_Sprite
    Mesh=None

	Tag='PixieMarker'
    CollisionRadius=50 
    CollisionHeight=5
    bCollideActors=False
	bBlockActors=False
	bBlockPlayers=False
    bCollideWorld=False
	bHidden=True

	groupName='None'

}
