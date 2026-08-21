//===============================================================================
//  [skGinnyPrivet] 
//===============================================================================

class skGinnyPrivet extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGinnyPrivetMesh MODELFILE=models\skGinnyPrivet.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGinnyPrivetMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skGinnyPrivetAnims ANIMFILE=models\skGinnyPrivet.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skGinnyPrivetMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGinnyPrivetMesh ANIM=skGinnyPrivetAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skGinnyPrivetAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skGinnyPrivetTex0  FILE=TEXTURES\HP2GINNY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyPrivetTex1  FILE=TEXTURES\HP2GINNY_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyPrivetTex2  FILE=TEXTURES\Gin_could_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyPrivetTex3  FILE=TEXTURES\Gin_Books_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyPrivetMesh NUM=0 TEXTURE=skGinnyPrivetTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyPrivetMesh NUM=1 TEXTURE=skGinnyPrivetTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyPrivetMesh NUM=2 TEXTURE=skGinnyPrivetTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyPrivetMesh NUM=3 TEXTURE=skGinnyPrivetTex3

// Original material [0] is [Ginny_SKIN00] SkinIndex: 0 Bitmap: HP2GINNY_SKIN00.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [1] is [Ginny_SKIN01] SkinIndex: 1 Bitmap: HP2GINNY_SKIN01.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [2] is [Ginny_SKIN02] SkinIndex: 2 Bitmap: Gin_could_128.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [3] is [Ginny_SKIN03] SkinIndex: 3 Bitmap: Gin_Books_128.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
