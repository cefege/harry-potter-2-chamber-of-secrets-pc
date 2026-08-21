//===============================================================================
//  [skProfMcGonagall] 
//===============================================================================

class skProfMcGonagall extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skProfMcGonagallMesh MODELFILE=models\skProfMcGonagall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skProfMcGonagallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skProfMcGonagallAnims ANIMFILE=models\skProfMcGonagall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skProfMcGonagallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skProfMcGonagallMesh ANIM=skProfMcGonagallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skProfMcGonagallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skProfMcGonagallTex0  FILE=TEXTURES\HP2_McGONAGALL_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfMcGonagallTex1  FILE=TEXTURES\HP2_McGONAGALL_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfMcGonagallTex2  FILE=TEXTURES\HP2_McGONAGALL_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfMcGonagallTex3  FILE=TEXTURES\HP2_McGONAGALL_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skProfMcGonagallMesh NUM=0 TEXTURE=skProfMcGonagallTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfMcGonagallMesh NUM=1 TEXTURE=skProfMcGonagallTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfMcGonagallMesh NUM=2 TEXTURE=skProfMcGonagallTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfMcGonagallMesh NUM=3 TEXTURE=skProfMcGonagallTex3

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2_McGONAGALL_SKIN00.bmp  Path: C:\potter\Characters\HP2\ProfMcGonagall 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2_McGONAGALL_SKIN01.bmp  Path: C:\potter\Characters\HP2\ProfMcGonagall 
// Original material [2] is [SKIN02.MASKED] SkinIndex: 2 Bitmap: HP2_McGONAGALL_SKIN02.bmp  Path: C:\potter\Characters\HP2\ProfMcGonagall 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: HP2_McGONAGALL_SKIN03.bmp  Path: C:\potter\Characters\HP2\ProfMcGonagall 
