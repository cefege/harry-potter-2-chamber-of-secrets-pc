//===============================================================================
//  [skHogFrontTorch] 
//===============================================================================

class skHogFrontTorch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skHogFrontTorchMesh MODELFILE=models\skHogFrontTorch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHogFrontTorchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHogFrontTorchAnims ANIMFILE=models\skHogFrontTorch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHogFrontTorchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHogFrontTorchMesh ANIM=skHogFrontTorchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHogFrontTorchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHogFrontTorchTex0  FILE=TEXTURES\HogFrontTorch.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHogFrontTorchMesh NUM=0 TEXTURE=skHogFrontTorchTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: HogFrontTorch.bmp  Path: C:\HP2 Art\Textures\Hogwarts Exterior 


defaultproperties
{
    Mesh=skHogFrontTorchMesh
    DrawType=DT_Mesh
    bStatic=False
}

