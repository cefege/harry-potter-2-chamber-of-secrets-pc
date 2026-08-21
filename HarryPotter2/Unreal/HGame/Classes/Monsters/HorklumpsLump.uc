// Class Name  : HorklumpsLump
//
// Created on  : 06/24/2002
// Authored by : Michael Lamkerovich
// 
// Description : HorklumpsLump AI. Can be picked up and thrown by Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class HorklumpsLump extends HProp;

function Touch( actor other )
{
	Super.Touch(other);

	if(other != playerHarry)
		return;

	playerHarry.TakeDamage( 1, self, Location, vect(0,0,0), '');

	spawn(class'PoisonCloud',self,,location, rotation);
	Destroy();
}

function HitWall(vector HitNormal, actor Wall)
{
	playerHarry.clientMessage("Hit wall ...........................hit");
	spawn(class'PoisonCloud',self,,location, rotation);
	Destroy();

}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skHorklumpChunkMesh'

	CollisionRadius=2
	CollisionHeight=2

	bBlockActors=False
	bBlockPlayers=False
	bCollideWorld=True

	Physics=PHYS_Walking	// Physics=PHYS_Falling
	bBounce=true

	DrawScale=1.2
}
