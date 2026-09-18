//===============================================================================
//  [ArmorWholeSuit] 
//===============================================================================

class ArmorWholeSuit extends HDecoration;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skArmorWholeSuitMesh'
     DrawScale=1.5
     CollisionRadius=24
     CollisionWidth=11
     CollisionHeight=60
     CollideType=CT_Box
}
