//===============================================================================
//  [skpinklady] 
//===============================================================================

class skpinklady extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skpinkladyMesh MODELFILE=models\skpinklady.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skpinkladyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skpinkladyAnims ANIMFILE=models\skpinklady.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skpinkladyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skpinkladyMesh ANIM=skpinkladyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skpinkladyAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skpinkladyTex0  FILE=TEXTURES\HP2PINKL_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skpinkladyTex1  FILE=TEXTURES\HP2PINKL_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skpinkladyTex2  FILE=TEXTURES\HP2PINKL_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skpinkladyMesh NUM=0 TEXTURE=skpinkladyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skpinkladyMesh NUM=1 TEXTURE=skpinkladyTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skpinkladyMesh NUM=2 TEXTURE=skpinkladyTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2PINKL_SKIN00.bmp  Path: C:\~Work\Harry Potter\HP2 Characters\HP2_PinkLady 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2PINKL_SKIN01.bmp  Path: C:\~Work\Harry Potter\HP2 Characters\HP2_PinkLady 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2PINKL_SKIN02.bmp  Path: C:\~Work\Harry Potter\HP2 Characters\HP2_PinkLady 
