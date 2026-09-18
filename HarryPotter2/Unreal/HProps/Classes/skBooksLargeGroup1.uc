//===============================================================================
//  [skBooksLargeGroup1] 
//===============================================================================

class skBooksLargeGroup1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksLargeGroup1Mesh MODELFILE=models\skBooksLargeGroup1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksLargeGroup1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksLargeGroup1Anims ANIMFILE=models\skBooksLargeGroup1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksLargeGroup1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksLargeGroup1Mesh ANIM=skBooksLargeGroup1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksLargeGroup1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksLargeGroup1Tex0  FILE=TEXTURES\BookRow2_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksLargeGroup1Mesh NUM=0 TEXTURE=skBooksLargeGroup1Tex0

// Original material [0] is [Material #8] SkinIndex: 0 Bitmap: BookRow2_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Large Group Row_Stack 


defaultproperties
{
    Mesh=skBooksLargeGroup1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

