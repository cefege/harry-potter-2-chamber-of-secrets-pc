//===============================================================================
//  [skArmTorchSepia] 
//===============================================================================

class skArmTorchSepia extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skArmTorchSepiaMesh MODELFILE=models\skArmTorchSepia.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skArmTorchSepiaMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skArmTorchSepiaAnims ANIMFILE=models\skArmTorchSepia.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skArmTorchSepiaMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skArmTorchSepiaMesh ANIM=skArmTorchSepiaAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skArmTorchSepiaAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skArmTorchSepiaTex0  FILE=TEXTURES\TorchArmSepia.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skArmTorchSepiaMesh NUM=0 TEXTURE=skArmTorchSepiaTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: TorchArmSepia.bmp  Path: C:\Harry Potter 2\ART\Objects\Candles_n_Candle_Sticks\Wall Arm Torch 


defaultproperties
{
    Mesh=skArmTorchSepiaMesh
    DrawType=DT_Mesh
    bStatic=False
}

