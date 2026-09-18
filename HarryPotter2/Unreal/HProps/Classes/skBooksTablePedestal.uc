//===============================================================================
//  [skBooksTablePedestal] 
//===============================================================================

class skBooksTablePedestal extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksTablePedestalMesh MODELFILE=models\skBooksTablePedestal.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksTablePedestalMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksTablePedestalAnims ANIMFILE=models\skBooksTablePedestal.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksTablePedestalMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksTablePedestalMesh ANIM=skBooksTablePedestalAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksTablePedestalAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksTablePedestalTex0  FILE=TEXTURES\bookped2_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksTablePedestalMesh NUM=0 TEXTURE=skBooksTablePedestalTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: bookped2_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Book on Pedestal\Desktop Display Pedestal 


defaultproperties
{
    Mesh=skBooksTablePedestalMesh
    DrawType=DT_Mesh
    bStatic=False
}

