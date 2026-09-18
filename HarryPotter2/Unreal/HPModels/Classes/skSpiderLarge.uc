//===============================================================================
//  [skSpiderLarge] 
//===============================================================================

class skSpiderLarge extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSpiderLargeMesh MODELFILE=models\skSpiderLarge.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpiderLargeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpiderLargeAnims ANIMFILE=models\skSpiderLarge.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpiderLargeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpiderLargeMesh ANIM=skSpiderLargeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpiderLargeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpiderLargeTex0  FILE=TEXTURES\HP2SPIDERL_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSpiderLargeTex1  FILE=TEXTURES\HP2SPIDERL_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSpiderLargeTex2  FILE=TEXTURES\HP2SPIDERL_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpiderLargeMesh NUM=0 TEXTURE=skSpiderLargeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skSpiderLargeMesh NUM=1 TEXTURE=skSpiderLargeTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skSpiderLargeMesh NUM=2 TEXTURE=skSpiderLargeTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SPIDERL_SKIN00.bmp  Path: C:\hp2_characters\spiders 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2SPIDERL_SKIN01.bmp  Path: C:\hp2_characters\spiders 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2SPIDERL_SKIN02.bmp  Path: C:\hp2_characters\spiders 
