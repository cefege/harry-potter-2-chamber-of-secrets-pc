//===============================================================================
//  [skBooksFloorPedestal] 
//===============================================================================

class skBooksFloorPedestal extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksFloorPedestalMesh MODELFILE=models\skBooksFloorPedestal.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksFloorPedestalMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksFloorPedestalAnims ANIMFILE=models\skBooksFloorPedestal.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksFloorPedestalMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksFloorPedestalMesh ANIM=skBooksFloorPedestalAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksFloorPedestalAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksFloorPedestalTex0  FILE=TEXTURES\bkpedstl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksFloorPedestalMesh NUM=0 TEXTURE=skBooksFloorPedestalTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: bkpedstl_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Book on Pedestal\Tall Book Pedestal 


defaultproperties
{
    Mesh=skBooksFloorPedestalMesh
    DrawType=DT_Mesh
    bStatic=False
}

