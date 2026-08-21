//===============================================================================
//  [skSheetTest1] 
//===============================================================================

class skSheetTest1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSheetTest1Mesh MODELFILE=models\skSheetTest1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSheetTest1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSheetTest1Anims ANIMFILE=models\skSheetTest1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSheetTest1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSheetTest1Mesh ANIM=skSheetTest1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSheetTest1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSheetTest1Tex0  FILE=TEXTURES\RoyalTapestry2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSheetTest1Mesh NUM=0 TEXTURE=skSheetTest1Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: RoyalTapestry2.bmp  Path: C:\HP2 Art\Textures\Patterns 


defaultproperties
{
    Mesh=skSheetTest1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

