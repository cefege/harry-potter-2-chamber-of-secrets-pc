//===============================================================================
//  [Armoire] 
//===============================================================================

class Armoire extends HFurniture;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skArmoireMesh'
     CollisionRadius=40
     CollisionWidth=18
     CollisionHeight=60
     CollideType=CT_Box
}
