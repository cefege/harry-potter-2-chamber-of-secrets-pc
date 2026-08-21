//===============================================================================
//  [skspikybushNoThorns] 
//===============================================================================

class spikybushNoThorns extends HChar;


auto state Wilted
{
	event AnimEnd()
	{
		// When wilted, players can walk over
//	   bBlockPlayers = false;
	   SetCollision(false,false,false);

		// Don't know if the following line is completely necessary.
		// trying to fix problem where withered plants block spells being done on normal plants
		SetCollisionSize(0, 0);
	}

begin:
	log("spiky bush wilting...");
    //bprojtarget=false;

	playAnim('wither');

	Sleep(1); // hold on for the first second of animation before playing sound effect

	PlaySound ( sound'HPSounds.Critters_sfx.spiky_bush_wilt', SLOT_None);
}



defaultproperties
{
    Mesh=skspikybushnothornsMesh
    DrawType=DT_Mesh
    bStatic=False
	PHYSICS=PHYS_None

	//eVulnerableToSpell=SPELL_Incendio
	bprojtarget=false
	bCollideActors=false
    bCollideWorld=false
    bBlockActors=false
    bBlockPlayers=false
	bdirectional=true
	bAlignBottom=true
	AmbientGlow=65
	drawScale=1.0
	collisionHeight=48	// 20*2.4
	collisionRadius=48	// 20*2.4
}

