//===============================================================================
//  [skHarryIsGoyle] 
//===============================================================================

class skHarryIsGoyle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHarryIsGoyleMesh MODELFILE=models\skHarryIsGoyle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHarryIsGoyleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHarryIsGoyleAnims ANIMFILE=models\skHarryIsGoyle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHarryIsGoyleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHarryIsGoyleMesh ANIM=skHarryIsGoyleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHarryIsGoyleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHarryIsGoyleTex0  FILE=TEXTURES\HP2GOYLE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryIsGoyleTex1  FILE=TEXTURES\HP2GOYLE_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHarryIsGoyleTex2  FILE=TEXTURES\HP2HARRY_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryIsGoyleMesh NUM=0 TEXTURE=skHarryIsGoyleTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryIsGoyleMesh NUM=1 TEXTURE=skHarryIsGoyleTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryIsGoyleMesh NUM=2 TEXTURE=skHarryIsGoyleTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2GOYLE_SKIN00.bmp  Path: C:\hp2_characters\HP2_Goyle 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2GOYLE_SKIN01.bmp  Path: C:\hp2_characters\HP2_Goyle 
// Original material [2] is [SKIN02.MASKED] SkinIndex: 2 Bitmap: HP2HARRY_SKIN03.bmp  Path: C:\hp2_characters\hp2_harry 
