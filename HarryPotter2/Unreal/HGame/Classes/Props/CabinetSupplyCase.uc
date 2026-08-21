//===============================================================================
//  [CabinetSupplyCase] 
//===============================================================================

class CabinetSupplyCase extends HFurniture;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skCabinetSupplyCaseMesh'
     DrawScale=1.2
     CollisionRadius=41
     CollisionWidth=18
     CollisionHeight=72
     CollideType=CT_Box
}
