//===============================================================================
//  [BooksTablePedestal] 
//===============================================================================

class BooksTablePedestal extends HBooks;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBooksTablePedestalMesh'
     DrawScale=1.1
     CollisionRadius=23
     CollisionWidth=17
     CollisionHeight=7
     CollideType=CT_Box
}
