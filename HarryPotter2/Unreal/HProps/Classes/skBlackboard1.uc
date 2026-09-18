//===============================================================================
//  [skBlackboard1] 
//===============================================================================

class skBlackboard1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBlackboard1Mesh MODELFILE=models\skBlackboard1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBlackboard1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBlackboard1Anims ANIMFILE=models\skBlackboard1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBlackboard1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBlackboard1Mesh ANIM=skBlackboard1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBlackboard1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBlackboard1Tex0  FILE=TEXTURES\Blackboard_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBlackboard1Mesh NUM=0 TEXTURE=skBlackboard1Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: Blackboard_128.bmp  Path: C:\Harry Potter\ART\Objects\Blackboards\Standup 


defaultproperties
{
    Mesh=skBlackboard1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

