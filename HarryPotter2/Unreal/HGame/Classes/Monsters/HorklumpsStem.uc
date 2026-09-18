// Class Name  : HorklumpsStem
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : HorklumpsStem AI (doesn't do anything but sit there)
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class HorklumpsStem extends HProp;

var vector v;

auto state wilt
{
	begin:

	sleep(2.0);

	playAnim('die');
	finishAnim();

}


defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skhorklumpsStemMesh'
     AmbientGlow=65
     CollisionRadius=5
     CollisionHeight=7
	 eVulnerableToSpell=SPELL_None
	 bBlockActors=False
	 bCollideWorld=True
	 bCollideActors=False
	 Physics=PHYS_Falling
}
