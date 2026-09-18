
// Class Name  : AragogHome
//
// Created on  : 08/05/2002
// Authored by : Janet Weddle
// 
// Description : AragogHome. The center of the lower pit. This is where Aragog returns to 
//				 after every physical attack on Harry.
//
//				 Placement: 4594.837,-2815.698,-1856
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogHome extends Actor;


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
