//===============================================================================
//  [skQuidPlayerM] 
//===============================================================================

class skQuidPlayerM extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skQuidPlayerMMesh MODELFILE=models\skQuidPlayerM.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skQuidPlayerMMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skQuidPlayerMMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skQuidPlayerMMesh ANIM=skHarryQuidAnims

#EXEC TEXTURE IMPORT NAME=skQuidPlayerM_GTex0  FILE=TEXTURES\MQUID1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skQuidPlayerM_STex0  FILE=TEXTURES\MQUID2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skQuidPlayerM_RTex0  FILE=TEXTURES\MQUID3_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skQuidPlayerM_HTex0  FILE=TEXTURES\MQUID4_SKIN00.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skQuidPlayerM_Tex1  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skQuidPlayerMMesh NUM=0 TEXTURE=skQuidPlayerM_GTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skQuidPlayerMMesh NUM=1 TEXTURE=skQuidPlayerM_Tex1

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: MQUID1_SKIN00.bmp  Path: C:\~Work\Harry Potter\Characters\MQuidditch 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: QUID_SKIN01.bmp  Path: C:\~Work\Harry Potter\Characters\MQuidditch 

