//===============================================================================
//  [skOliverWoodQuid] 
//===============================================================================

class skOliverWoodQuid extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skOliverWoodQuidMesh MODELFILE=models\skOliverWoodQuid.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skOliverWoodQuidMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skOliverWoodQuidMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skOliverWoodQuidMesh ANIM=skHarryQuidAnims

#EXEC TEXTURE IMPORT NAME=skOliverWoodQuidTex0  FILE=TEXTURES\HP2HARRYQ_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodQuidTex1  FILE=TEXTURES\HP2HARRYQ_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodQuidTex2  FILE=TEXTURES\HP2OLIVER_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodQuidTex3  FILE=TEXTURES\HP2OLIVER_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodQuidTex4  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodQuidMesh NUM=0 TEXTURE=skOliverWoodQuidTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodQuidMesh NUM=1 TEXTURE=skOliverWoodQuidTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodQuidMesh NUM=2 TEXTURE=skOliverWoodQuidTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodQuidMesh NUM=3 TEXTURE=skOliverWoodQuidTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodQuidMesh NUM=4 TEXTURE=skOliverWoodQuidTex4

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRYQ_SKIN00.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HARRYQ_SKIN01.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2OLIVER_SKIN00.bmp  Path: C:\potter\Characters\HP2\OliverWood 
// Original material [3] is [SKIN03] SkinIndex: 3 Bitmap: HP2OLIVER_SKIN01.bmp  Path: C:\potter\Characters\HP2\OliverWood 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: QUID_SKIN01.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
