//===============================================================================
//  [FlipendoVaseMing] 
//===============================================================================

class FlipendoVaseMing extends FlipendoVaseBronze;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skFlipendoVaseMingMesh'
     CollisionRadius=15
     CollisionHeight=19
	//brokentype=class'FlipendoVaseMingBroken';
	ShardType=class'FlipendoVaseMingShard'
	brokentypeMesh=SkeletalMesh'HProps.skFlipendoVaseMingBrokenMesh'

}
