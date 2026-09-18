//===============================================================================
//  [skBooksOwlBookendBooks] 
//===============================================================================

class skBooksOwlBookendBooks extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksOwlBookendBooksMesh MODELFILE=models\skBooksOwlBookendBooks.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksOwlBookendBooksMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksOwlBookendBooksAnims ANIMFILE=models\skBooksOwlBookendBooks.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksOwlBookendBooksMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksOwlBookendBooksMesh ANIM=skBooksOwlBookendBooksAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksOwlBookendBooksAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksOwlBookendBooksTex0  FILE=TEXTURES\BooksRow_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksOwlBookendBooksMesh NUM=0 TEXTURE=skBooksOwlBookendBooksTex0

// Original material [0] is [Material #8] SkinIndex: 0 Bitmap: BooksRow_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Owl Bookend Books 


defaultproperties
{
    Mesh=skBooksOwlBookendBooksMesh
    DrawType=DT_Mesh
    bStatic=False
}

