//===============================================================================
//  [skgoldchest] 
//===============================================================================

class skgoldchest extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skgoldchestMesh MODELFILE=models\skgoldchest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skgoldchestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skgoldchestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skgoldchestMesh ANIM=skbronzechestAnims

#EXEC TEXTURE IMPORT NAME=skgoldchestTex0  FILE=TEXTURES\gldtrunk_128.BMP  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skgoldchestMesh NUM=0 TEXTURE=skgoldchestTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: gldtrunk_128.BMP  Path: C:\Nathan 
