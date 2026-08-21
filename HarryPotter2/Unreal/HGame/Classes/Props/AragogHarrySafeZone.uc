
// Class Name  : AragogHarrySafeZone
//
// Created on  : 08/05/2002
// Authored by : Janet Weddle
// 
// Description : AragogHarrySafeZone. When Phase Two begins Harry is moved to this spot during the 
//				 the cutscene so he's out of the way of the falling mover. 
//
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogHarrySafeZone extends Actor;


defaultproperties
{
	drawType=DT_SPRITE
    DrawScale=1
    CollisionRadius=10
    CollisionHeight=10

	bCollideWorld=False
	bCollideActors=False
	bBlockPlayers=False
	bBlockActors=False

	eVulnerableToSpell=SPELL_None

	bHidden=True

}
