//===============================================================================
//  [skHarryToGoyle] 
//===============================================================================

class skHarryToGoyle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHarryToGoyleMesh MODELFILE=models\skHarryToGoyle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHarryToGoyleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHarryToGoyleAnims ANIMFILE=models\skHarryToGoyle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHarryToGoyleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHarryToGoyleMesh ANIM=skHarryToGoyleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHarryToGoyleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHarryToGoyleTex0  FILE=TEXTURES\HP2HARRY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryToGoyleTex1  FILE=TEXTURES\HP2HARRY_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryToGoyleTex2  FILE=TEXTURES\HP2HARRY_SKIN05.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryToGoyleTex3  FILE=TEXTURES\HP2HARRY_SKIN03.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryToGoyleTex4  FILE=TEXTURES\juiceglass_skin06.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryToGoyleMesh NUM=0 TEXTURE=skHarryToGoyleTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryToGoyleMesh NUM=1 TEXTURE=skHarryToGoyleTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryToGoyleMesh NUM=2 TEXTURE=skHarryToGoyleTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryToGoyleMesh NUM=3 TEXTURE=skHarryToGoyleTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryToGoyleMesh NUM=4 TEXTURE=skHarryToGoyleTex4

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HARRY_SKIN00.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HARRY_SKIN01.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2HARRY_SKIN05.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: HP2HARRY_SKIN03.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [4] is [skin04] SkinIndex: 4 Bitmap: juiceglass_skin06.bmp  Path: C:\hp2_characters\hp2_harry 
