
// Class Name  : SpikyPlantStem
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : SpikyPlantStem AI (doesn't do anything but sit there)
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpikyPlantStem extends HProp;

var float scaleIncrement;

function Tick(float DeltaTime)
{

	Super.Tick(DeltaTime);

	if ( drawScale > 0.75 )
	{
		drawScale -= scaleIncrement;
	}
	else if ( drawScale < 0.75 )
	{
		drawScale = 0.75;
	}

}

defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skSpikyPlantStemMesh'
     AmbientGlow=65
     CollisionRadius=20
     CollisionHeight=4
	 eVulnerableToSpell=SPELL_None
	 bBlockActors=False
	 bCollideWorld=False
	 bCollideActors=False
	 scaleIncrement=0.1
}
