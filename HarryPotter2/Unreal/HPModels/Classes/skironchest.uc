//===============================================================================
//  [skironchest] 
//===============================================================================

class skironchest extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skironchestMesh MODELFILE=models\skironchest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skironchestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skironchestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skironchestMesh ANIM=skbronzechestAnims

#EXEC TEXTURE IMPORT NAME=skironchestTex0  FILE=TEXTURES\irntrunk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skironchestMesh NUM=0 TEXTURE=skironchestTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: irntrunk_128.bmp  Path: C:\Nathan 

