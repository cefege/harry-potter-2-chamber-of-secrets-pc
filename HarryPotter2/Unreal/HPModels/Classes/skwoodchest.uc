//===============================================================================
//  [skwoodchest] 
//===============================================================================

class skwoodchest extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skwoodchestMesh MODELFILE=models\skwoodchest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skwoodchestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skwoodchestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skwoodchestMesh ANIM=skbronzechestAnims

#EXEC TEXTURE IMPORT NAME=skwoodchestTex0  FILE=TEXTURES\wodtrunk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skwoodchestMesh NUM=0 TEXTURE=skwoodchestTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: wodtrunk_128.bmp  Path: C:\Nathan 

