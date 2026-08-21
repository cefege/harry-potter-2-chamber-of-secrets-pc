//===============================================================================
//  [skGeorgeWeasley] 
//===============================================================================

class skGeorgeWeasley extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGeorgeWeasleyMesh MODELFILE=models\skGeorgeWeasley.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGeorgeWeasleyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skGeorgeWeasleyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGeorgeWeasleyMesh ANIM=skGeorgeWeasleyAnims

#EXEC TEXTURE IMPORT NAME=skGeorgeWeasleyTex0  FILE=TEXTURES\HP2GEORGE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGeorgeWeasleyTex1  FILE=TEXTURES\HP2GEORGE_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWeasleyMesh NUM=0 TEXTURE=skGeorgeWeasleyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWeasleyMesh NUM=1 TEXTURE=skGeorgeWeasleyTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2GEORGE_SKIN00.bmp  Path: C:\hp2_characters\hp2GeorgeWeasley 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2GEORGE_SKIN01.bmp  Path: C:\hp2_characters\hp2GeorgeWeasley 
