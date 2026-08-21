//===============================================================================
//  [skArmTorch] 
//===============================================================================

class skArmTorch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skArmTorchMesh MODELFILE=models\skArmTorch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skArmTorchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skArmTorchAnims ANIMFILE=models\skArmTorch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skArmTorchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skArmTorchMesh ANIM=skArmTorchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skArmTorchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skArmTorchTex0  FILE=TEXTURES\armtorchtexturemap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skArmTorchMesh NUM=0 TEXTURE=skArmTorchTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: armtorchtexturemap.bmp  Path: C:\HP2_master\Skurge\objects 


defaultproperties
{
    Mesh=skArmTorchMesh
    DrawType=DT_Mesh
    bStatic=False
}

