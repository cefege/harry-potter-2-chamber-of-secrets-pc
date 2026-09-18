//===============================================================================
//  [BooksSmallBook] 
//===============================================================================

class BooksSmallBook extends HBooks;

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBooksSmallBookMesh'
     CollisionRadius=10
     CollisionWidth=15
     CollisionHeight=5
     CollideType=CT_Box
}
