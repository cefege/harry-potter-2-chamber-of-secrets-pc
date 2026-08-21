//===============================================================================
//  [WWellBlueBottle] 
//===============================================================================

class WWellBlueBottle extends WiggenWell;

defaultproperties
{
	Mesh=SkeletalMesh'HProps.skBottlePotionBlueMesh'
	DrawType=DT_Mesh

	classStatusGroup=Class'HGame.StatusGroupPotions'
	classStatusItem=Class'HGame.StatusItemWiggenWell'

	bBlockActors=false
	bBlockPlayers=false
}

