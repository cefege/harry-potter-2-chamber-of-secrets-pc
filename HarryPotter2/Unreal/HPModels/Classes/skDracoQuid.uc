//===============================================================================
//  [skDracoQuid] 
//===============================================================================

class skDracoQuid extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skDracoQuidMesh MODELFILE=models\skDracoQuid.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDracoQuidMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skDracoQuidMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDracoQuidMesh ANIM=skHarryQuidAnims

#EXEC TEXTURE IMPORT NAME=skDracoQuidTex0  FILE=TEXTURES\HP2DRACOQ_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDracoQuidTex1  FILE=TEXTURES\HP2DRACOQ_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDracoQuidTex2  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDracoQuidMesh NUM=0 TEXTURE=skDracoQuidTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skDracoQuidMesh NUM=1 TEXTURE=skDracoQuidTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skDracoQuidMesh NUM=2 TEXTURE=skDracoQuidTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2DRACOQ_SKIN00.bmp  Path: C:\potter\Characters\HP2\DracoQuid 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2DRACOQ_SKIN01.bmp  Path: C:\potter\Characters\HP2\DracoQuid 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: QUID_SKIN01.bmp  Path: C:\potter\Characters\HP2\DracoQuid 
