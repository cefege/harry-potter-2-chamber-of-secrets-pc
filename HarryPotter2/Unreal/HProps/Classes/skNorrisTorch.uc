//===============================================================================
//  [skNorrisTorch] 
//===============================================================================

class skNorrisTorch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skNorrisTorchMesh MODELFILE=models\skNorrisTorch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skNorrisTorchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skNorrisTorchAnims ANIMFILE=models\skNorrisTorch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skNorrisTorchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skNorrisTorchMesh ANIM=skNorrisTorchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skNorrisTorchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skNorrisTorchTex0  FILE=TEXTURES\NorrisTorch_128_02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skNorrisTorchMesh NUM=0 TEXTURE=skNorrisTorchTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: NorrisTorch_128_02.bmp  Path: C:\HP2_master\Grandstaircase 


defaultproperties
{
    Mesh=skNorrisTorchMesh
    DrawType=DT_Mesh
    bStatic=False
}

