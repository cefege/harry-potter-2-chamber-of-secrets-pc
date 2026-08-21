//===============================================================================
//  [skTomRiddleSepia] 
//===============================================================================

class skTomRiddleSepia extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skTomRiddleSepiaMesh MODELFILE=models\skTomRiddleSepia.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTomRiddleSepiaMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skTomRiddleSepiaMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTomRiddleSepiaMesh ANIM=skTomRiddleAnims

#EXEC TEXTURE IMPORT NAME=skTomRiddleSepiaTex0  FILE=TEXTURES\HP2TOMRS_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skTomRiddleSepiaTex1  FILE=TEXTURES\HP2TOMRS_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTomRiddleSepiaMesh NUM=0 TEXTURE=skTomRiddleSepiaTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skTomRiddleSepiaMesh NUM=1 TEXTURE=skTomRiddleSepiaTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2TOMRS_SKIN00.bmp  Path: C:\hp2_characters\HP2_TomRiddle 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2TOMRS_SKIN01.bmp  Path: C:\hp2_characters\HP2_TomRiddle 
