//===============================================================================
//  [skGeorgeWQuid] 
//===============================================================================

class skGeorgeWQuid extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGeorgeWQuidMesh MODELFILE=models\skgeorgeWQuid.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGeorgeWQuidMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skGeorgeWQuidMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGeorgeWQuidMesh ANIM=skHarryQuidAnims

#EXEC TEXTURE IMPORT NAME=skGeorgeWQuidTex0  FILE=TEXTURES\HP2HARRYQ_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGeorgeWQuidTex1  FILE=TEXTURES\HP2HARRYQ_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGeorgeWQuidTex2  FILE=TEXTURES\HP2OLIVER_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGeorgeWQuidTex3  FILE=TEXTURES\HP2FRED_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGeorgeWQuidTex4  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWQuidMesh NUM=0 TEXTURE=skGeorgeWQuidTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWQuidMesh NUM=1 TEXTURE=skGeorgeWQuidTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWQuidMesh NUM=2 TEXTURE=skGeorgeWQuidTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWQuidMesh NUM=3 TEXTURE=skGeorgeWQuidTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skGeorgeWQuidMesh NUM=4 TEXTURE=skGeorgeWQuidTex4

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRYQ_SKIN00.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HARRYQ_SKIN01.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2OLIVER_SKIN01.bmp  Path: C:\potter\Characters\HP2\OliverWood 
// Original material [3] is [SKIN03] SkinIndex: 3 Bitmap: HP2FRED_SKIN00.bmp  Path: C:\potter\Characters\HP2\George_Fred 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: QUID_SKIN01.bmp  Path: C:\~Work\Harry Potter\HP2 Characters\HP2_HarryQuid 
