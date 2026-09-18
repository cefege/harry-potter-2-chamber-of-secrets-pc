//===============================================================================
//  [FlipendoVaseGreen] 
//===============================================================================

class FlipendoVaseGreen extends FlipendoVaseBronze;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skFlipendoVaseGreenMesh'
     CollisionRadius=15
     CollisionHeight=19
	//brokentype=class'FlipendoVaseGreenBroken';
	ShardType=class'FlipendoVaseGreenShard'
	brokentypeMesh=SkeletalMesh'HProps.skFlipendoVaseGreenBrokenMesh'

}
