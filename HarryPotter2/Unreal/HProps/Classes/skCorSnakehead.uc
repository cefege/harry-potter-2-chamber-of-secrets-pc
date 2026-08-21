//===============================================================================
//  [skCorSnakehead] 
//===============================================================================

class skCorSnakehead extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCorSnakeheadMesh MODELFILE=models\skCorSnakehead.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCorSnakeheadMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCorSnakeheadAnims ANIMFILE=models\skCorSnakehead.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCorSnakeheadMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCorSnakeheadMesh ANIM=skCorSnakeheadAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCorSnakeheadAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCorSnakeheadTex0  FILE=TEXTURES\CorSnakehead_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skCorSnakeheadTex1  FILE=TEXTURES\CorSnakehead_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCorSnakeheadMesh NUM=0 TEXTURE=skCorSnakeheadTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skCorSnakeheadMesh NUM=1 TEXTURE=skCorSnakeheadTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: CorSnakehead_SKIN01.bmp  Path: C:\HP2_master\Corridor\psd 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: CorSnakehead_SKIN00.bmp  Path: C:\HP2_master\Corridor\psd 


defaultproperties
{
    Mesh=skCorSnakeheadMesh
    DrawType=DT_Mesh
    bStatic=False
}

