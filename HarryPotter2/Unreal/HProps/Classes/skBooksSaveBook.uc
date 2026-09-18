//===============================================================================
//  [skBooksSaveBook] 
//===============================================================================

class skBooksSaveBook extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksSaveBookMesh MODELFILE=models\skBooksSaveBook.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksSaveBookMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksSaveBookAnims ANIMFILE=models\skBooksSaveBook.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksSaveBookMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksSaveBookMesh ANIM=skBooksSaveBookAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksSaveBookAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksSaveBookTex0  FILE=TEXTURES\floatbok_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBooksSaveBookTex1  FILE=TEXTURES\floatbok_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksSaveBookMesh NUM=0 TEXTURE=skBooksSaveBookTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksSaveBookMesh NUM=1 TEXTURE=skBooksSaveBookTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: floatbok_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Save Book 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: floatbok_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Save Book 


defaultproperties
{
    Mesh=skBooksSaveBookMesh
    DrawType=DT_Mesh
    bStatic=False
}

