//===============================================================================
//  [DeskTeacher] 
//===============================================================================

class DeskTeacher extends HFurniture;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skDeskTeacherMesh'
     CollisionRadius=25
     CollisionWidth=50
     CollisionHeight=27
     CollideType=CT_Box
}
