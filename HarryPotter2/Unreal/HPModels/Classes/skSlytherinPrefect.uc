//===============================================================================
//  [skSlytherinPrefect] 
//===============================================================================

class skSlytherinPrefect extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSlytherinPrefectMesh MODELFILE=models\skSlytherinPrefect.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSlytherinPrefectMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skSlytherinPrefectMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSlytherinPrefectMesh ANIM=skGeorgeWeasleyAnims

#EXEC TEXTURE IMPORT NAME=skSlytherinPrefectTex0  FILE=TEXTURES\HP2SLYTHP_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSlytherinPrefectTex1  FILE=TEXTURES\HP2SLYTHP_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSlytherinPrefectMesh NUM=0 TEXTURE=skSlytherinPrefectTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skSlytherinPrefectMesh NUM=1 TEXTURE=skSlytherinPrefectTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SLYTHP_SKIN00.bmp  Path: C:\potter\Characters\HP2\PercyWeasley 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2SLYTHP_SKIN01.bmp  Path: C:\potter\Characters\HP2\PercyWeasley 
