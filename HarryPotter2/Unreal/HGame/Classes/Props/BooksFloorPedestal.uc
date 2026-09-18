//===============================================================================
//  [BooksFloorPedestal] 
//===============================================================================

class BooksFloorPedestal extends HBooks;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBooksFloorPedestalMesh'
     DrawScale=1.1
     CollisionWidth=18
     CollisionHeight=21
     CollideType=CT_Box
}
