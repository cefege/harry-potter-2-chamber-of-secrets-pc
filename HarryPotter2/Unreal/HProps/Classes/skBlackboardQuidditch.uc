//===============================================================================
//  [skBlackboardQuidditch] 
//===============================================================================

class skBlackboardQuidditch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBlackboardQuidditchMesh MODELFILE=models\skBlackboardQuidditch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBlackboardQuidditchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBlackboardQuidditchAnims ANIMFILE=models\skBlackboardQuidditch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBlackboardQuidditchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBlackboardQuidditchMesh ANIM=skBlackboardQuidditchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBlackboardQuidditchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBlackboardQuidditchTex0  FILE=TEXTURES\QuiditchBlackboard.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBlackboardQuidditchMesh NUM=0 TEXTURE=skBlackboardQuidditchTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: QuiditchBlackboard.bmp  Path: C:\Harry Potter 2\ART\Objects\Blackboards\Standup 


defaultproperties
{
    Mesh=skBlackboardQuidditchMesh
    DrawType=DT_Mesh
    bStatic=False
}

