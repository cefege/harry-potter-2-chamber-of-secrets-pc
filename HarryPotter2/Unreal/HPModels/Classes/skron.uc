//===============================================================================
//  [skron] 
//===============================================================================

class skron extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skronMesh MODELFILE=models\skron.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skronMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skronMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skronMesh ANIM=skGenMaleAnims

#EXEC TEXTURE IMPORT NAME=skronTex0  FILE=TEXTURES\HP2RON_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skronTex1  FILE=TEXTURES\HP2RON_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skronMesh NUM=0 TEXTURE=skronTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skronMesh NUM=1 TEXTURE=skronTex1

#exec MESH WEAPONATTACH MESH=skRonMesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skRonMesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2RON_SKIN00.bmp  Path: C:\potter\Characters\HP2\Ron 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2RON_SKIN01.bmp  Path: C:\potter\Characters\HP2\Ron 
