//===============================================================================
//  [BenchWithArms] 
//===============================================================================

class BenchWithArms extends HFurniture;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBenchWithArmsMesh'
     CollisionRadius=80
     CollisionWidth=24
     CollisionHeight=37
     CollideType=CT_Box
}
