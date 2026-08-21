//===============================================================================
//  [skGinnyLucious] 
//===============================================================================

class skGinnyLucious extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGinnyLuciousMesh MODELFILE=models\skGinnyLucious.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGinnyLuciousMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skGinnyLuciousAnims ANIMFILE=models\skGinnyLucious.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skGinnyLuciousMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGinnyLuciousMesh ANIM=skGinnyLuciousAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skGinnyLuciousAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex0  FILE=TEXTURES\HP2GINNY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex1  FILE=TEXTURES\HP2GINNY_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex2  FILE=TEXTURES\Gin_could_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex3  FILE=TEXTURES\Gin_Books_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex4  FILE=TEXTURES\HP2LUC_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex5  FILE=TEXTURES\HP2LUC_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyLuciousTex6  FILE=TEXTURES\diarybook_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=0 TEXTURE=skGinnyLuciousTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=1 TEXTURE=skGinnyLuciousTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=2 TEXTURE=skGinnyLuciousTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=3 TEXTURE=skGinnyLuciousTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=4 TEXTURE=skGinnyLuciousTex4
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=5 TEXTURE=skGinnyLuciousTex5
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyLuciousMesh NUM=6 TEXTURE=skGinnyLuciousTex6

// Original material [0] is [Ginny_SKIN00] SkinIndex: 0 Bitmap: HP2GINNY_SKIN00.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [1] is [Ginny_SKIN01] SkinIndex: 1 Bitmap: HP2GINNY_SKIN01.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [2] is [Ginny_SKIN02] SkinIndex: 2 Bitmap: Gin_could_128.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [3] is [Ginny_SKIN03] SkinIndex: 3 Bitmap: Gin_Books_128.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: HP2LUC_SKIN00.bmp  Path: C:\hp2_characters\HP2_LuciousMalfoy 
// Original material [5] is [SKIN05] SkinIndex: 5 Bitmap: HP2LUC_SKIN01.bmp  Path: C:\hp2_characters\HP2_LuciousMalfoy 
// Original material [6] is [SKIN06] SkinIndex: 6 Bitmap: diarybook_128.bmp  Path: C:\HP2_Objects\Books\Single Small Book 
