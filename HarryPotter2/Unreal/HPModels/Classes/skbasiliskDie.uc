//===============================================================================
//  [skbasiliskDie] 
//===============================================================================

class skbasiliskDie extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbasiliskDieMesh MODELFILE=models\skbasiliskDie.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbasiliskDieMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbasiliskDieAnims ANIMFILE=models\skbasiliskDie.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbasiliskDieMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbasiliskDieMesh ANIM=skbasiliskDieAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbasiliskDieAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbasiliskDieTex0  FILE=TEXTURES\HP2BASILISK_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskDieTex1  FILE=TEXTURES\HP2BASILISK_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskDieTex2  FILE=TEXTURES\HP2BASILISK_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskDieTex3  FILE=TEXTURES\HP2BASILISK_SKIN03.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskDieTex4  FILE=TEXTURES\HP2BASILISK_SKIN04.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskDieTex5  FILE=TEXTURES\diarytexturemap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskDieMesh NUM=0 TEXTURE=skbasiliskDieTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskDieMesh NUM=1 TEXTURE=skbasiliskDieTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskDieMesh NUM=2 TEXTURE=skbasiliskDieTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskDieMesh NUM=3 TEXTURE=skbasiliskDieTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskDieMesh NUM=4 TEXTURE=skbasiliskDieTex4
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskDieMesh NUM=5 TEXTURE=skbasiliskDieTex5

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2BASILISK_SKIN00.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2BASILISK_SKIN01.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2BASILISK_SKIN02.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [3] is [SKIN04] SkinIndex: 4 Bitmap: HP2BASILISK_SKIN04.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [4] is [Skin05] SkinIndex: 5 Bitmap: diarytexturemap.bmp  Path: C:\hp2_characters\HP2_basilisk 
