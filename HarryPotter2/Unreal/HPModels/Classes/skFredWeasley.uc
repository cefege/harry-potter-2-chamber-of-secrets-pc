//===============================================================================
//  [skFredWeasley] 
//===============================================================================

class skFredWeasley extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skFredWeasleyMesh MODELFILE=models\skFredWeasley.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFredWeasleyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skFredWeasleyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFredWeasleyMesh ANIM=skGeorgeWeasleyAnims

#EXEC TEXTURE IMPORT NAME=skFredWeasleyTex0  FILE=TEXTURES\HP2FRED_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFredWeasleyTex1  FILE=TEXTURES\HP2FRED_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFredWeasleyMesh NUM=0 TEXTURE=skFredWeasleyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFredWeasleyMesh NUM=1 TEXTURE=skFredWeasleyTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2FRED_SKIN00.bmp  Path: C:\hp2_characters\hp2GeorgeWeasley 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2FRED_SKIN01.bmp  Path: C:\hp2_characters\hp2GeorgeWeasley 
