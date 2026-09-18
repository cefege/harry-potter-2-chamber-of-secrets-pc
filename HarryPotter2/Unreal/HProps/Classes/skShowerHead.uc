//===============================================================================
//  [skShowerHead] 
//===============================================================================

class skShowerHead extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skShowerHeadMesh MODELFILE=models\skShowerHead.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skShowerHeadMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skShowerHeadAnims ANIMFILE=models\skShowerHead.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skShowerHeadMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skShowerHeadMesh ANIM=skShowerHeadAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skShowerHeadAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skShowerHeadTex0  FILE=TEXTURES\GreenHouseShowerhead.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skShowerHeadMesh NUM=0 TEXTURE=skShowerHeadTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: GreenHouseShowerhead.bmp  Path: C:\Harry Potter\ART\Objects\Tools\Greenhouse Shower Head 


defaultproperties
{
    Mesh=skShowerHeadMesh
    DrawType=DT_Mesh
    bStatic=False
}

