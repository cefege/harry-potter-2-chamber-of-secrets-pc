//===============================================================================
//  [skAragog] 
//===============================================================================

class skAragog extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skAragogMesh MODELFILE=models\skAragog.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skAragogMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skAragogAnims ANIMFILE=models\skAragog.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skAragogMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skAragogMesh ANIM=skAragogAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skAragogAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skAragogTex0  FILE=TEXTURES\HP2ARAGOG_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skAragogTex1  FILE=TEXTURES\HP2ARAGOG_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skAragogTex2  FILE=TEXTURES\HP2ARAGOG_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogMesh NUM=0 TEXTURE=skAragogTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogMesh NUM=1 TEXTURE=skAragogTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogMesh NUM=2 TEXTURE=skAragogTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2ARAGOG_SKIN00.bmp  Path: C:\hp2_characters\HP2_Aragog 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2ARAGOG_SKIN01.bmp  Path: C:\hp2_characters\HP2_Aragog 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2ARAGOG_SKIN02.bmp  Path: C:\hp2_characters\HP2_Aragog 
