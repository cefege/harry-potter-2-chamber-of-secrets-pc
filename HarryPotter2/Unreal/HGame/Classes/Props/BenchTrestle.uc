//===============================================================================
//  [BenchTrestle] 
//===============================================================================

class BenchTrestle extends HFurniture;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBenchTrestleMesh'
     CollisionRadius=50
     CollisionWidth=20
     CollisionHeight=18
     CollideType=CT_Box
}
