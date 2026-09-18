//===============================================================================
//  [skSheetTest] 
//===============================================================================

class skSheetTest extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSheetTestMesh MODELFILE=models\skSheetTest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSheetTestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSheetTestAnims ANIMFILE=models\skSheetTest.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSheetTestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSheetTestMesh ANIM=skSheetTestAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSheetTestAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSheetTestTex0  FILE=TEXTURES\RoyalTapestry2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSheetTestMesh NUM=0 TEXTURE=skSheetTestTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: RoyalTapestry2.bmp  Path: C:\HP2 Art\Textures\Patterns 


defaultproperties
{
    Mesh=skSheetTestMesh
    DrawType=DT_Mesh
    bStatic=False
}

