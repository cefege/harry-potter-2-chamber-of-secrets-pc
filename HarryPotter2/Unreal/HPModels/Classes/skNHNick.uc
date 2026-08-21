//===============================================================================
//  [skNHNick] 
//===============================================================================

class skNHNick extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skNHNickMesh MODELFILE=models\skNHNick.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skNHNickMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skNHNickAnims ANIMFILE=models\skNHNick.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skNHNickMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skNHNickMesh ANIM=skNHNickAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skNHNickAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skNHNickTex0  FILE=TEXTURES\NICK_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skNHNickTex1  FILE=TEXTURES\NICK_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skNHNickTex2  FILE=TEXTURES\NICK_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skNHNickMesh NUM=0 TEXTURE=skNHNickTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skNHNickMesh NUM=1 TEXTURE=skNHNickTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skNHNickMesh NUM=2 TEXTURE=skNHNickTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: NICK_SKIN00.bmp  Path: C:\potter\Characters\HP2\NearlyHeadlessNick 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: NICK_SKIN01.bmp  Path: C:\potter\Characters\HP2\NearlyHeadlessNick 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: NICK_SKIN02.bmp  Path: C:\potter\Characters\HP2\NearlyHeadlessNick 
