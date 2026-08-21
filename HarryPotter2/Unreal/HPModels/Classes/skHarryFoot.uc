//===============================================================================
//  [skHarryFoot] 
//===============================================================================

class skHarryFoot extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHarryFootMesh MODELFILE=models\skHarryFoot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHarryFootMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHarryFootAnims ANIMFILE=models\skHarryFoot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHarryFootMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHarryFootMesh ANIM=skHarryFootAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHarryFootAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHarryFootTex0  FILE=TEXTURES\HP2HARRY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryFootTex1  FILE=TEXTURES\HP2HARRY_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryFootMesh NUM=0 TEXTURE=skHarryFootTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryFootMesh NUM=1 TEXTURE=skHarryFootTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRY_SKIN00.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HARRY_SKIN01.bmp  Path: C:\hp2_characters\hp2_harry 
