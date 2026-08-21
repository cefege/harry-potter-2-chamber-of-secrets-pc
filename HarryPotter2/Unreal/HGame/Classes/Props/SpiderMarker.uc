
// Class Name  : SpiderMarker
//
// Created on  : 05/24/2002
// Authored by : Janet Weddle
// 
// Description : SpiderMarker marks spots where spiders will go. Put them on the walls
//				 and on the floor. There is a boolean bCenter that is set to false by
//				 default. If bCenter is true the spider will return to this spot every
//				 time it leaves another spot. Set this if you want the spider to always
//				 go to a certain spot (in front of a door)
//
// NOTE:	   : 07/03/2002
//				 SpiderMarker was derived from HiddenHPawn but there was an issue where Harry
//				 was trying to get a fix on them to throw a spell. Eli suggested that if they
//				 were derived from Actor Harry would ignore them. So some of this code is taken
//				 directly from HiddenHPawn so they would look the same
//
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpiderMarker extends Actor;

#exec Texture Import File=Textures\Hidpawn.pcx Name=HiddenPawn Mips=Off Flags=2

// *** Variables

var() bool bCenter;		// This is a center spot where the spider will always go
var() string groupName; // The spider will only go to spots with the same name
var() int numLargeSpiders;	// The total number of Large spiders needed inside this spawner (Aragog only)
var() int numSmallSpiders;  // The total number of small spiders needed inside this spawner (Aragog only)
var() float checkSpiderTime;  // How often check for number of spiders (Aragog only)

var float checkTime;

var SpiderSpawner largeSpawner;
var SpiderSpawner smallSpawner;

var PlayerPawn playerHarry;
var Aragog spider;


// *** Constants
 
const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

function PreBeginPlay()
{
	super.PreBeginPlay();

	//Is this needed?   YES!! It is!  Just putting the four defaults down in defaultproperties WONT make the actor
	// move normally through the world with SetLocation().  You have to call SetCollision to make the actor be TRULY non colliding.
	SetCollision(,,);
	bCollideWorld = false;

	foreach AllActors( class'PlayerPawn', playerHarry)
		break;

	foreach AllActors( class'Aragog', spider )
		break;

}

function postBeginPlay()
{
	SetCollision( true, false, false );
	if ( numSmallSpiders != 0 || numLargeSpiders != 0 )
	{
		FindClosestSpawner();
		checkTime = checkSpiderTime;
	}

}

function FindClosestSpawner()
{
	local int counter;
	local float dist, farthestDist;
	local SpiderSpawner ss;

	farthestDist = 99999999999;

	if ( numSmallSpiders != 0 )
	{
		foreach AllActors( class'SpiderSpawner', ss )
		{
			if ( ss.bSmallSpiders == true )
			{
				if ( Vsize(ss.location - location) < farthestDist )
				{
					smallSpawner = ss;
					farthestDist = Vsize(ss.location - location);
				}
			}
		}
	}

	farthestDist = 99999999999;

	if ( numLargeSpiders != 0 )
	{
		foreach AllActors( class'SpiderSpawner', ss )
		{
			if ( ss.bSmallSpiders == false )
			{	
				if ( Vsize(ss.location - location) < farthestDist )
				{
					largeSpawner = ss;
					farthestDist = Vsize(ss.location - location);
				}
			}
		}
	}

}

function incrementNumSmallSpiders()
{
	if ( numsmallSpiders != 0 )
	{
		numsmallSpiders++;
	}
}

function incrementNumLargeSpiders()
{
	if ( numLargeSpiders != 0 )
	{
		numLargeSpiders++;
	}
}

function disableMarker()
{
	// disable the marker so it won't spawn anymore spiders
	numLargeSpiders = 0;
	numSmallSpiders = 0;

	gotoState('disabled');
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

auto state justWait
{
	function Tick(float DeltaTime)
	{
		local int smallFound, largeFound;
		local SpiderSmall smallSpider;
		local SpiderLarge largeSpider;

		if ( numSmallSpiders != 0 || numLargeSpiders != 0 )
		{
			checkTime -= DeltaTime;

			if ( checkTime <= 0 )
			{

				checkTime = checkSpiderTime;

				if ( numSmallSpiders != 0 )
				{
					foreach AllActors( class'SpiderSmall', smallSpider )
					{
						if ( smallSpider.groupName == groupName )
						{	
							smallFound++;
						}
					}
				}

				if ( numLargeSpiders != 0 )
				{
					foreach AllActors( class'SpiderLarge', largeSpider )
					{
						if ( largeSpider.groupName == groupName )
						{	
							largeFound++;
						}
					}
				}

				if ( largeFound < numLargeSpiders )
				{
					playerHarry.clientMessage("Spawning Large Spiders :  "$numLargeSpiders-largeFound);
					largeSpawner.SpawnRandomSpiders(numLargeSpiders-largeFound);

					// Loop through all of the Large spiders in that group and send out a call so they
					// will update their list of friends.
//					foreach AllActors( class'SpiderLarge', largeSpider )
//					{
//						if ( largeSpider.groupName == groupName )
//						{	
//							largeSpider.UpdateFriendsList();
//						}
//					}
				}

				if ( smallFound < numSmallSpiders )
				{
					playerHarry.clientMessage("Spawning Small Spiders :  "$numSmallSpiders-smallFound);
					smallSpawner.SpawnRandomSpiders(numSmallSpiders-smallFound);
				}


			}
		}
	}

	begin:

}

state disabled
{
	begin:
}

defaultproperties
{
	Texture=Texture'HGame.HiddenPawn'
    DrawType=DT_Sprite
    Mesh=None

	Tag='SpiderMarker'
    CollisionRadius=50 
    CollisionHeight=5
    bCollideActors=False
	bBlockActors=False
	bBlockPlayers=False
	bBlockCamera=false
    bCollideWorld=False
	bHidden=True

	checkSpiderTime=15

	bCenter=False
	groupName='None'

}
