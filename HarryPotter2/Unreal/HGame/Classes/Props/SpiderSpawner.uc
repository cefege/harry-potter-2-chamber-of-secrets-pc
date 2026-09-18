
// Class Name  : SpiderSpawner
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : SpiderSpawner. When triggered this will spawn up to five spiders. You can type
//				 whatever number you want but it will max at 5.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpiderSpawner extends HiddenHpawn;

 
var int counter;
var rotator spiderRotation;
var float spiderYaw;


var() int numSpiders;
var() bool bSmallSpiders;

var spiderSmall smallSpider;
var spiderLarge largeSpider;


struct SpiderSpawnerParams
{	
	var() float  normalSpeed;
	var() float  attackSpeed;
	var() bool   canWander;  
	var() bool   waitForTrigger;
	var() float  forwardDistance;
	var() string groupName;
	var() int    iRotation;
	var() float	 nextSpiderDelay;
	var() float  drawingScale;		// The drawScale of the spiders (used only from spawner)
	var() float  jumpingDistanceFromHarry;	// If greater than this distance use the jumping anim to get close to Harry. 
	var() int    numSpellsLargeSpider;		// numSpellsDefault
	var() float  leaveSmallDeadSpider;	// leaveDeadSpider
};



var() SpiderSpawnerParams theSpiders[5];


//degrees & 0x0000ffff  ???

// *** Constants

const		BOOL_DEBUG_AI	= true;	// if true this will spit out debug AI info


function postBeginPlay()
{

	bCollideWorld = true;
}

function Trigger(actor Other, pawn EventInstigator)
{

	gotoState('SpawnSomeSpiders');
}


function touch (actor other)
{
	Super.Touch(other);

}

function bump( actor other)
{
	touch(other);
}


function SpawnRandomSpiders(int ns)
{
	local int randNum, numberOfSpiders;
	local int Spidercounter;
	local spiderSmall smSpider;
	local spiderLarge lgSpider;

	numberOfSpiders = ns;

	for (Spidercounter=0; Spidercounter<numberOfSpiders; Spidercounter++)
	{
		randNum = rand(numSpiders);

		if ( bSmallSpiders == true )
		{
			spiderRotation = rotation;

			spiderYaw = (theSpiders[randNum].iRotation * (0x4000/90.0));
			spiderRotation.Yaw += spiderYaw;
			spiderRotation.Yaw = spiderRotation.Yaw & 0x0000ffff;

			smSpider = spawn( class'SpiderSmall',self,,location,spiderRotation);

			smSpider.normalSpeed = theSpiders[randNum].normalSpeed;
			smSpider.attackSpeed = theSpiders[randNum].attackSpeed;
			smSpider.groundSpeed = theSpiders[randNum].normalSpeed;
			smSpider.canWander = theSpiders[randNum].canWander;
			smSpider.waitForTrigger = theSpiders[randNum].waitForTrigger;
			smSpider.forwardDistance = theSpiders[randNum].forwardDistance;
			smSpider.DrawScale = theSpiders[randNum].drawingScale;
			smSpider.jumpingDistanceFromHarry = theSpiders[randNum].jumpingDistanceFromHarry;
			smSpider.groupName = theSpiders[randNum].groupName;
			smSpider.numSpellsDefault = theSpiders[randNum].numSpellsLargeSpider;
			smSpider.leaveDeadSpider = theSpiders[randNum].leaveSmallDeadSpider;
			smSpider.bDespawnable = true;
		}
		else
		{
			spiderRotation = rotation;

			spiderYaw = (theSpiders[randNum].iRotation * (0x4000/90.0));
			spiderRotation.Yaw += spiderYaw;
			spiderRotation.Yaw = spiderRotation.Yaw & 0x0000ffff;

			lgSpider = spawn( class'SpiderLarge',self,,location,spiderRotation);

			lgSpider.normalSpeed = theSpiders[randNum].normalSpeed;
			lgSpider.attackSpeed = theSpiders[randNum].attackSpeed;
			lgSpider.groundSpeed = theSpiders[randNum].normalSpeed;
			lgSpider.canWander = theSpiders[randNum].canWander;
			lgSpider.waitForTrigger = theSpiders[randNum].waitForTrigger;
			lgSpider.forwardDistance = theSpiders[randNum].forwardDistance;
			lgSpider.DrawScale = theSpiders[randNum].drawingScale;
			lgSpider.jumpingDistanceFromHarry = theSpiders[randNum].jumpingDistanceFromHarry;
			lgSpider.groupName = theSpiders[randNum].groupName;
			lgSpider.numSpellsDefault = theSpiders[randNum].numSpellsLargeSpider;
			lgSpider.leaveDeadSpider = theSpiders[randNum].leaveSmallDeadSpider;
			lgSpider.bDespawnable = true;

			switch (rand(2))
			{
				case 0: lgSpider.ePreAttackAnim = ATTACK_JUMP; break;
				case 1: lgSpider.ePreAttackAnim = ATTACK_REAR; break;
			}
		}

	}

	gotoState('spiderSpawnerWait');

}


auto state spiderSpawnerWait
{

	begin:

}

state SpawnSomeSpiders
{

	begin:

	counter = 0;

//	for ( counter=0; counter < numSpiders; counter++ )
	while ( counter < numSpiders )
	{

		if ( bSmallSpiders == true )
		{
			spiderRotation = rotation;

			spiderYaw = (theSpiders[counter].iRotation * (0x4000/90.0));
			spiderRotation.Yaw += spiderYaw;
			spiderRotation.Yaw = spiderRotation.Yaw & 0x0000ffff;

			smallSpider = spawn( class'SpiderSmall',self,,location,spiderRotation);

			smallSpider.normalSpeed = theSpiders[counter].normalSpeed;
			smallSpider.attackSpeed = theSpiders[counter].attackSpeed;
			smallSpider.groundSpeed = theSpiders[counter].normalSpeed;
			smallSpider.canWander = theSpiders[counter].canWander;
			smallSpider.waitForTrigger = theSpiders[counter].waitForTrigger;
			smallSpider.forwardDistance = theSpiders[counter].forwardDistance;
			smallSpider.DrawScale = theSpiders[counter].drawingScale;
			smallSpider.jumpingDistanceFromHarry = theSpiders[counter].jumpingDistanceFromHarry;
			smallSpider.groupName = theSpiders[counter].groupName;
			smallSpider.numSpellsDefault = theSpiders[counter].numSpellsLargeSpider;
			smallSpider.leaveDeadSpider = theSpiders[counter].leaveSmallDeadSpider;
		}
		else
		{
			spiderRotation = rotation;

			spiderYaw = (theSpiders[counter].iRotation * (0x4000/90.0));
			spiderRotation.Yaw += spiderYaw;
			spiderRotation.Yaw = spiderRotation.Yaw & 0x0000ffff;

			largeSpider = spawn( class'SpiderLarge',self,,location,spiderRotation);

			largeSpider.normalSpeed = theSpiders[counter].normalSpeed;
			largeSpider.attackSpeed = theSpiders[counter].attackSpeed;
			largeSpider.groundSpeed = theSpiders[counter].normalSpeed;
			largeSpider.canWander = theSpiders[counter].canWander;
			largeSpider.waitForTrigger = theSpiders[counter].waitForTrigger;
			largeSpider.forwardDistance = theSpiders[counter].forwardDistance;
			largeSpider.DrawScale = theSpiders[counter].drawingScale;
			largeSpider.jumpingDistanceFromHarry = theSpiders[counter].jumpingDistanceFromHarry;
			largeSpider.groupName = theSpiders[counter].groupName;
			largeSpider.numSpellsDefault = theSpiders[counter].numSpellsLargeSpider;
			largeSpider.leaveDeadSpider = theSpiders[counter].leaveSmallDeadSpider;
		}

		counter++;

		sleep(theSpiders[counter].nextSpiderDelay);

	}
 
	gotoState('spiderSpawnerWait');
 
}

defaultproperties
{
	 drawType=DT_Sprite
	 Physics=PHYS_None
	 Mass=10
     CollisionRadius=10
     CollisionHeight=10

	 bCollideActors=True
     bCollideWorld=True
	 bBlockPlayers=False 
	 bBlockActors=False

	 numSpiders=5
	 bSmallSpiders=True

	 theSpiders(0)=(normalSpeed=75,attackSpeed=100,canWander=True,waitForTrigger=False,forwardDistance=0,iRotation=0,nextSpiderDelay=0.5,drawingScale=1,jumpingDistanceFromHarry=250,numSpellsLargeSpider=1,leaveSmallDeadSpider=0.2
 	 theSpiders(1)=(normalSpeed=75,attackSpeed=100,canWander=True,waitForTrigger=False,forwardDistance=0,iRotation=0,nextSpiderDelay=0.5,drawingScale=1,jumpingDistanceFromHarry=250,numSpellsLargeSpider=1,leaveSmallDeadSpider=0.2
 	 theSpiders(2)=(normalSpeed=75,attackSpeed=100,canWander=True,waitForTrigger=False,forwardDistance=0,iRotation=0,nextSpiderDelay=0.5,drawingScale=1,jumpingDistanceFromHarry=250,numSpellsLargeSpider=1,leaveSmallDeadSpider=0.2
	 theSpiders(3)=(normalSpeed=75,attackSpeed=100,canWander=True,waitForTrigger=False,forwardDistance=0,iRotation=0,nextSpiderDelay=0.5,drawingScale=1,jumpingDistanceFromHarry=250,numSpellsLargeSpider=1,leaveSmallDeadSpider=0.2
	 theSpiders(4)=(normalSpeed=75,attackSpeed=100,canWander=True,waitForTrigger=False,forwardDistance=0,iRotation=0,nextSpiderDelay=0.5,drawingScale=1,jumpingDistanceFromHarry=250,numSpellsLargeSpider=1,leaveSmallDeadSpider=0.2

}



