
// Class Name  : AragogCenterWeb
//
// Created on  : 07/17/2002
// Authored by : Janet Weddle
// 
// Description : AragogCenterWeb. The web that Aragog sits on in Phase Two of the Aragog level.
//				 This is just for the touch so Aragog knows to bite Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogCenterWeb extends Actor;


// variables
var Aragog spider;


//**********************************************************************
function postBeginPlay()
{
	Super.postBeginPlay();

	foreach AllActors( class'Aragog', spider )
		break;

}

function Touch(actor other)
{
	if ( other.IsA('Harry') )
	{
		spider.bOnMyWeb = true;
	}
}

function bump(actor other)
{
	touch(other);
}


//**********************************************************************//

defaultproperties
{
	drawType=DT_SPRITE
    DrawScale=1
    CollisionRadius=465
    CollisionHeight=100

	bCollideWorld=True
	bCollideActors=True
	bBlockPlayers=True
	bBlockActors=False

	eVulnerableToSpell=SPELL_None

	bHidden=True

}
