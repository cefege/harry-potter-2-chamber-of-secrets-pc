//===============================================================================
//  [BathroomToiletTank] 
//===============================================================================

class BathroomToiletTank extends HBathroom;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBathroomToiletTankMesh'
     DrawScale=1.04
     CollisionRadius=7
     CollisionWidth=16
     CollisionHeight=38
     CollideType=CT_Box
}
