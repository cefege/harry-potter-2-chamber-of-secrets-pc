//===============================================================================
//  [skBooksBookRow1] 
//===============================================================================

class skBooksBookRow1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksBookRow1Mesh MODELFILE=models\skBooksBookRow1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksBookRow1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksBookRow1Anims ANIMFILE=models\skBooksBookRow1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksBookRow1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksBookRow1Mesh ANIM=skBooksBookRow1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksBookRow1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksBookRow1Tex0  FILE=TEXTURES\BookStck2_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksBookRow1Mesh NUM=0 TEXTURE=skBooksBookRow1Tex0

// Original material [0] is [Material #7] SkinIndex: 0 Bitmap: BookStck2_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Six Book Row 


defaultproperties
{
    Mesh=skBooksBookRow1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

