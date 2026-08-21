//===============================================================================
//  [ChickenLeg] 
//===============================================================================

class ChickenLeg extends HProp;

defaultproperties
{
    Mesh=skChickenLegMesh
    DrawType=DT_Mesh

    CollisionRadius=10
    CollisionHeight=10

	Physics=PHYS_Falling

	// pickup related
	bBlockActors=false
	bBlockPlayers=false
	bCollideActors=false
	bBlockCamera=false
}

