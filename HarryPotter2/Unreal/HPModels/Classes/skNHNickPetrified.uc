//===============================================================================
//  [skNHNickPetrified] 
//===============================================================================

class skNHNickPetrified extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skNHNickPetrifiedMesh MODELFILE=models\skNHNickPetrified.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skNHNickPetrifiedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skNHNickPetrifiedAnims ANIMFILE=models\skNHNickPetrified.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skNHNickPetrifiedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skNHNickPetrifiedMesh ANIM=skNHNickPetrifiedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skNHNickPetrifiedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skNHNickPetrifiedTex0  FILE=TEXTURES\HP2NICKP_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skNHNickPetrifiedTex1  FILE=TEXTURES\HP2NICKP_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skNHNickPetrifiedTex2  FILE=TEXTURES\HP2NICKP_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skNHNickPetrifiedMesh NUM=0 TEXTURE=skNHNickPetrifiedTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skNHNickPetrifiedMesh NUM=1 TEXTURE=skNHNickPetrifiedTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skNHNickPetrifiedMesh NUM=2 TEXTURE=skNHNickPetrifiedTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2NICKP_SKIN00.bmp  Path: C:\potter\Characters\HP2\NearlyHeadlessNick 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2NICKP_SKIN01.bmp  Path: C:\potter\Characters\HP2\NearlyHeadlessNick 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2NICKP_SKIN02.bmp  Path: C:\potter\Characters\HP2\NearlyHeadlessNick 
