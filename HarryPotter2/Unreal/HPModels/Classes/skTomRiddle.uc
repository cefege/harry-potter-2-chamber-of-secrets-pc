//===============================================================================
//  [skTomRiddle] 
//===============================================================================

class skTomRiddle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skTomRiddleMesh MODELFILE=models\skTomRiddle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTomRiddleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skTomRiddleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTomRiddleMesh ANIM=skTomRiddleAnims

#EXEC TEXTURE IMPORT NAME=skTomRiddleTex0  FILE=TEXTURES\HP2TOMR_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skTomRiddleTex1  FILE=TEXTURES\HP2TOMR_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTomRiddleMesh NUM=0 TEXTURE=skTomRiddleTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skTomRiddleMesh NUM=1 TEXTURE=skTomRiddleTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2TOMR_SKIN00.bmp  Path: C:\hp2_characters\HP2_TomRiddle 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2TOMR_SKIN01.bmp  Path: C:\hp2_characters\HP2_TomRiddle 
