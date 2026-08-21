//===============================================================================
//  [WWellGreenBottle] 
//
//  Bottle that Harry holds when drinking a wiggenwell potion.
//
//===============================================================================

class WWellGreenBottle extends WiggenWell;

defaultproperties
{
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skharry_bottleMesh'
	classStatusGroup=Class'HGame.StatusGroupPotions'
	classStatusItem=Class'HGame.StatusItemWiggenWell'
	bBlockActors=false
	bBlockPlayers=false
}

