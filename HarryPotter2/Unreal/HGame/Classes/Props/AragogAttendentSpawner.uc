
// Class Name  : AragogAttendentSpawner
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : AragogAttendentSpawner. When triggered this will spawn Aragog attendent spiders
//				 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogAttendentSpawner extends HiddenHpawn;

 
var int counter;
var rotator spiderRotation;
var float spiderYaw;
var int Spidercounter;
var int numberOfSpiders;
var int numberOfAnchors;
var spiderAttendent atSpider;

var vector vRight;
var vector vTempOffset;
var vector vOffset;
var vector vOffsetSide;
var vector vOffsetFront;
var vector vOffsetUp;

struct AragogAttendentParams
{
	var() float	attackSpeed;				// speed of spider while attacking
	var() int	iRotation;					// rotation of the spider
	var() float	drawingScale;				// drawScale of the spider
	var() float damageAmount;				// the amount of damage the spider does
	var() vector offsetFromSpawner;			// where the spider will spawn in relation to the spawner
	var() float	jumpingDistanceFromHarry;	// If greater than this distance use the jumping anim to get close to Harry. 
	var() int	numSpells;					// numSpellsDefault
};

struct SpiderSpawnerParams
{	

	var() AragogAttendentParams theSpiders[4];
	var() int numberOfSpiders;				// The number of spiders to spawn when this many anchors are hit
	var() float	nextSpiderDelay;			// time to wait before spawning next spider
};

var() SpiderSpawnerParams theAnchors[8];


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

function SpawnSpiders(int a)
{
	numberOfAnchors = a;
	numberOfSpiders = theAnchors[numberOfAnchors].numberOfSpiders;

	vRight = vector(rotation) cross vect(0,0,1);

	gotoState('spawnSomeSpiders');
}

state spawnSomeSpiders
{
	begin:

cm("Number of spiders: " $numberofSpiders);


	for ( spiderCounter = 0; spiderCounter < numberOfSpiders; spiderCounter++ )
	{

		spiderRotation = rotation;

		spiderYaw = (theAnchors[numberOfAnchors].theSpiders[SpiderCounter].iRotation * (0x4000/90.0));
		spiderRotation.Yaw += spiderYaw;
		spiderRotation.Yaw = spiderRotation.Yaw & 0x0000ffff;

		
		vTempOffset = theAnchors[numberOfAnchors].theSpiders[SpiderCounter].offsetFromSpawner;

		vOffsetSide = vRight * vTempOffset.x;
		vOffsetFront = vector(rotation) * vTempOffset.y;
		vOffsetUp = vec(0,0,vTempOffset.z);

		vOffset = vOffSetSide + vOffsetFront + vOffsetUp;


		atSpider = spawn( class'SpiderAttendent',self,,location+vOffset,spiderRotation);

cm("Spawning a spiders at offset : " $vTempOffset);

		atSpider.attackSpeed = theAnchors[numberOfAnchors].theSpiders[SpiderCounter].attackSpeed;
		atSpider.DrawScale = theAnchors[numberOfAnchors].theSpiders[SpiderCounter].drawingScale;
		atSpider.fDamageAmount = theAnchors[numberOfAnchors].theSpiders[SpiderCounter].damageAmount;
		atSpider.jumpingDistanceFromHarry = theAnchors[numberOfAnchors].theSpiders[SpiderCounter].jumpingDistanceFromHarry;
		atSpider.numSpellsDefault = theAnchors[numberOfAnchors].theSpiders[SpiderCounter].numSpells;
		atSpider.bDespawnable = true;

		switch (rand(2))
		{
			case 0: atSpider.ePreAttackAnim = ATTACK_JUMP; break;
			case 1: atSpider.ePreAttackAnim = ATTACK_REAR; break;
		}

		sleep(theAnchors[numberOfAnchors].nextSpiderDelay);

	}
		
}


defaultproperties
{
	 drawType=DT_Sprite
	 Physics=PHYS_None
	 Mass=10
     CollisionRadius=10
     CollisionHeight=10

	 bCollideActors=False
     bCollideWorld=False
	 bBlockPlayers=False 
	 bBlockActors=False
	 bBlockCamera=false
}



