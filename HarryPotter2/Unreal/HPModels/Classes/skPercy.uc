//===============================================================================
//  [skPercy] 
//===============================================================================

class skPercy extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skPercyMesh MODELFILE=models\skPercy.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPercyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skPercyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPercyMesh ANIM=skGeorgeWeasleyAnims

#EXEC TEXTURE IMPORT NAME=skPercyTex0  FILE=TEXTURES\HP2PERCY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skPercyTex1  FILE=TEXTURES\HP2PERCY_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPercyMesh NUM=0 TEXTURE=skPercyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skPercyMesh NUM=1 TEXTURE=skPercyTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2PERCY_SKIN00.bmp  Path: C:\potter\Characters\HP2\PercyWeasley 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2PERCY_SKIN01.bmp  Path: C:\potter\Characters\HP2\PercyWeasley 
