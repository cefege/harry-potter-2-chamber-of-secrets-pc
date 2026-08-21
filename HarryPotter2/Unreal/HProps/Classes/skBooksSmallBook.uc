//===============================================================================
//  [skBooksSmallBook] 
//===============================================================================

class skBooksSmallBook extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksSmallBookMesh MODELFILE=models\skBooksSmallBook.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksSmallBookMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksSmallBookAnims ANIMFILE=models\skBooksSmallBook.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksSmallBookMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksSmallBookMesh ANIM=skBooksSmallBookAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksSmallBookAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksSmallBookTex0  FILE=TEXTURES\tranbook_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksSmallBookMesh NUM=0 TEXTURE=skBooksSmallBookTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: tranbook_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Single Small Book 


defaultproperties
{
    Mesh=skBooksSmallBookMesh
    DrawType=DT_Mesh
    bStatic=False
}

