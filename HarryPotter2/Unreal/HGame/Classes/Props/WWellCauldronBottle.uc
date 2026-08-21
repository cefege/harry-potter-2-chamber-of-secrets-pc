//===============================================================================
//  [WWellCauldronBottle.uc] 
//
//  Potion bottle that is spawned out of the mixing cauldron.
//
//===============================================================================

class WWellCauldronBottle extends WiggenWell;

defaultproperties
{
    DrawType=DT_Mesh
    Mesh=SkeletalMesh'HProps.skBottlePotionGreen1Mesh'
    CollisionRadius=8
    CollisionHeight=13
	classStatusGroup=Class'HGame.StatusGroupPotions'
	classStatusItem=Class'HGame.StatusItemWiggenWell'
	bBlockActors=false
	bBlockPlayers=false
}
