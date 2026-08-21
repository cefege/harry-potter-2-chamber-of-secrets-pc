//===============================================================================
//  [skemptyplantpot] 
//===============================================================================

class skemptyplantpot extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skemptyplantpotMesh MODELFILE=models\skemptyplantpot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skemptyplantpotMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skemptyplantpotAnims ANIMFILE=models\skemptyplantpot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skemptyplantpotMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skemptyplantpotMesh ANIM=skemptyplantpotAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skemptyplantpotAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skemptyplantpotTex0  FILE=TEXTURES\HP2PLANT_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skemptyplantpotMesh NUM=0 TEXTURE=skemptyplantpotTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2PLANT_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\EmptyPlantpot 
