//===============================================================================
//  [Telescope] 
//===============================================================================

class Telescope extends HDecoration;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skTelescopeMesh'
     CollisionRadius=26
     CollisionWidth=100
     CollisionHeight=140
     CollideType=CT_Box
}
