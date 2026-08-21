//===============================================================================
//  [skQuidPlayerF] 
//===============================================================================

class skQuidPlayerF extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skQuidPlayerFMesh MODELFILE=models\skQuidPlayerF.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skQuidPlayerFMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skQuidPlayerFMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skQuidPlayerFMesh ANIM=skHarryQuidAnims

#EXEC TEXTURE IMPORT NAME=skQuidPlayerF_GTex0  FILE=TEXTURES\FQUID1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skQuidPlayerF_STex0  FILE=TEXTURES\FQUID2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skQuidPlayerF_RTex0  FILE=TEXTURES\FQUID3_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skQuidPlayerF_HTex0  FILE=TEXTURES\FQUID4_SKIN00.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skQuidPlayerF_Tex1  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skQuidPlayerFMesh NUM=0 TEXTURE=skQuidPlayerF_GTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skQuidPlayerFMesh NUM=1 TEXTURE=skQuidPlayerF_Tex1

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: FQUID1_SKIN00.bmp  Path: C:\~Work\Harry Potter\Characters\FQuidditch 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: QUID_SKIN01.bmp  Path: C:\~Work\Harry Potter\Characters\FQuidditch 

