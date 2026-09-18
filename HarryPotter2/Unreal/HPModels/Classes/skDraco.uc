//===============================================================================
//  [skDraco] 
//===============================================================================

class skDraco extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skDracoMesh MODELFILE=models\skDraco.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDracoMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skDracoMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDracoMesh ANIM=skGenMaleAnims

#EXEC TEXTURE IMPORT NAME=skDracoTex0  FILE=TEXTURES\HP2DRACO_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDracoTex1  FILE=TEXTURES\HP2DRACO_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDracoMesh NUM=0 TEXTURE=skDracoTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skDracoMesh NUM=1 TEXTURE=skDracoTex1

#exec MESH WEAPONATTACH MESH=skDracoMesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skDracoMesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2DRACO_SKIN00.bmp  Path: C:\hp2_characters\HP2_DracoMalfoy 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2DRACO_SKIN01.bmp  Path: C:\hp2_characters\HP2_DracoMalfoy 
