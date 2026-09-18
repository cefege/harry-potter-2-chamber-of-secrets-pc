//===============================================================================
//  [skHarGin] 
//===============================================================================

class skHarGin extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHarGinMesh MODELFILE=models\skHarGin.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHarGinMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHarGinAnims ANIMFILE=models\skHarGin.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHarGinMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHarGinMesh ANIM=skHarGinAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHarGinAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHarGinTex0  FILE=TEXTURES\HP2HARRY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarGinTex1  FILE=TEXTURES\HP2HARRY_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarGinTex2  FILE=TEXTURES\HP2HARRY_SKIN05.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarGinTex3  FILE=TEXTURES\HP2GINNY_SKIN06.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarGinTex4  FILE=TEXTURES\HP2GINNY_SKIN07.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarGinTex5  FILE=TEXTURES\HP2HARRY_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHarGinMesh NUM=0 TEXTURE=skHarGinTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarGinMesh NUM=1 TEXTURE=skHarGinTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarGinMesh NUM=2 TEXTURE=skHarGinTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarGinMesh NUM=3 TEXTURE=skHarGinTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarGinMesh NUM=4 TEXTURE=skHarGinTex4
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarGinMesh NUM=5 TEXTURE=skHarGinTex5

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRY_SKIN00.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HARRY_SKIN01.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2HARRY_SKIN05.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [3] is [Ginny_SKIN03] SkinIndex: 3 Bitmap: HP2GINNY_SKIN06.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [4] is [Ginny_SKIN04] SkinIndex: 4 Bitmap: HP2GINNY_SKIN07.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [5] is [SKIN05.MASKED] SkinIndex: 5 Bitmap: HP2HARRY_SKIN03.bmp  Path: C:\hp2_characters\hp2_harry 
