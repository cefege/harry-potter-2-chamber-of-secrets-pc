//===============================================================================
//  [skharry] 
//===============================================================================

class skharry extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skharryMesh MODELFILE=models\skharry.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skharryMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skharryMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skharryMesh ANIM=skHarryAnims

#EXEC TEXTURE IMPORT NAME=skharryTex0  FILE=TEXTURES\HP2HARRY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryTex1  FILE=TEXTURES\HP2HARRY_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryTex2  FILE=TEXTURES\HP2HARRY_SKIN05.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryTex3  FILE=TEXTURES\HP2HARRY_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skharryMesh NUM=0 TEXTURE=skharryTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryMesh NUM=1 TEXTURE=skharryTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryMesh NUM=2 TEXTURE=skharryTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryMesh NUM=3 TEXTURE=skharryTex3

#exec MESH WEAPONATTACH MESH=skHarryMesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skHarryMesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRY_SKIN00.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2HARRY_SKIN01.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2HARRY_SKIN05.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: HP2HARRY_SKIN03.bmp  Path: C:\hp2_characters\hp2_harry 
