//===============================================================================
//  [WWellOrangeBottle] 
//===============================================================================

class WWellOrangeBottle extends WiggenWell;

defaultproperties
{
	Mesh=SkeletalMesh'HProps.skBottlePotionOrangeMesh'
	DrawType=DT_Mesh

	classStatusGroup=Class'HGame.StatusGroupPotions'
	classStatusItem=Class'HGame.StatusItemWiggenWell'

	bBlockActors=false
	bBlockPlayers=false
}

