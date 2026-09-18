//===============================================================================
//  [skharryquid] 
//===============================================================================

class skharryquid extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skharryquidMesh MODELFILE=models\skharryquid.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skharryquidMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skharryquidMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skharryquidMesh ANIM=skHarryQuidAnims

#EXEC TEXTURE IMPORT NAME=skharryquidTex0  FILE=TEXTURES\HP2HARRYQ_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryquidTex1  FILE=TEXTURES\HP2HARRYQ_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryquidTex2  FILE=TEXTURES\HP2HARRYQ_SKIN05.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryquidTex3  FILE=TEXTURES\HP2HARRY_SKIN03.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skharryquidTex4  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skharryquidMesh NUM=0 TEXTURE=skharryquidTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryquidMesh NUM=1 TEXTURE=skharryquidTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryquidMesh NUM=2 TEXTURE=skharryquidTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryquidMesh NUM=3 TEXTURE=skharryquidTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skharryquidMesh NUM=4 TEXTURE=skharryquidTex4

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRYQ_SKIN00.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HARRYQ_SKIN01.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2HARRYQ_SKIN05.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: HP2HARRY_SKIN03.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: QUID_SKIN01.bmp  Path: C:\potter\Characters\HP2\HarryQuid 
