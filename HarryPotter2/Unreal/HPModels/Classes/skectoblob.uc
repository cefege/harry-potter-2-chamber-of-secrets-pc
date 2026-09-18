//===============================================================================
//  [skectoblob] 
//===============================================================================

class skectoblob extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skectoblobMesh MODELFILE=models\skectoblob.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skectoblobMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skectoblobMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skectoblobMesh ANIM=skectoblobAnims

#EXEC TEXTURE IMPORT NAME=skectoblobTex0  FILE=TEXTURES\ectoplasm.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skectoblobMesh NUM=0 TEXTURE=skectoblobTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: ectoplasm.bmp  Path: C:\hp2_characters\HP2_EctoPlasma 
