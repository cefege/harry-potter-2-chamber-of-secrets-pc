//===============================================================================
//  [skbronzechest] 
//===============================================================================

class skbronzechest extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbronzechestMesh MODELFILE=models\skbronzechest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbronzechestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skbronzechestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbronzechestMesh ANIM=skbronzechestAnims

#EXEC TEXTURE IMPORT NAME=skbronzechestTex0  FILE=TEXTURES\brztrunk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbronzechestMesh NUM=0 TEXTURE=skbronzechestTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: brztrunk_128.bmp  Path: C:\Nathan 

