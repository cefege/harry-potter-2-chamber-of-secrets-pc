//===============================================================================
//  [skYoungAragog] 
//===============================================================================

class skYoungAragog extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skYoungAragogMesh MODELFILE=models\skYoungAragog.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skYoungAragogMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skYoungAragogAnims ANIMFILE=models\skYoungAragog.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skYoungAragogMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skYoungAragogMesh ANIM=skYoungAragogAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skYoungAragogAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skYoungAragogTex0  FILE=TEXTURES\HP2ARAGOG2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skYoungAragogTex1  FILE=TEXTURES\HP2ARAGOG2_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skYoungAragogTex2  FILE=TEXTURES\HP2ARAGOG2_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skYoungAragogMesh NUM=0 TEXTURE=skYoungAragogTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skYoungAragogMesh NUM=1 TEXTURE=skYoungAragogTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skYoungAragogMesh NUM=2 TEXTURE=skYoungAragogTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2ARAGOG2_SKIN00.bmp  Path: C:\hp2_characters\HP2_Aragog 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2ARAGOG2_SKIN01.bmp  Path: C:\hp2_characters\HP2_Aragog 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2ARAGOG2_SKIN02.bmp  Path: C:\hp2_characters\HP2_Aragog 
