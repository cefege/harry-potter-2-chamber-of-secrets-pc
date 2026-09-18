//===============================================================================
//  [skLockHarry] 
//===============================================================================

class skLockHarry extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skLockHarryMesh MODELFILE=models\skLockHarry.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLockHarryMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLockHarryAnims ANIMFILE=models\skLockHarry.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLockHarryMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLockHarryMesh ANIM=skLockHarryAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLockHarryAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLockHarryTex0  FILE=TEXTURES\HP2LOCK_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockHarryTex1  FILE=TEXTURES\HP2LOCK_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockHarryTex2  FILE=TEXTURES\HP2HARRY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockHarryTex3  FILE=TEXTURES\HP2HARRY_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockHarryTex4  FILE=TEXTURES\HP2HARRY_SKIN05.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockHarryTex5  FILE=TEXTURES\HP2HARRY_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLockHarryMesh NUM=0 TEXTURE=skLockHarryTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockHarryMesh NUM=1 TEXTURE=skLockHarryTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockHarryMesh NUM=2 TEXTURE=skLockHarryTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockHarryMesh NUM=3 TEXTURE=skLockHarryTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockHarryMesh NUM=4 TEXTURE=skLockHarryTex4
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockHarryMesh NUM=5 TEXTURE=skLockHarryTex5

// Original material [0] is [skin00] SkinIndex: 0 Bitmap: HP2LOCK_SKIN00.bmp  Path: C:\hp2_characters\HP2_lockhart 
// Original material [1] is [skin01] SkinIndex: 1 Bitmap: HP2LOCK_SKIN01.bmp  Path: C:\hp2_characters\HP2_lockhart 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2HARRY_SKIN00.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [3] is [SKIN03.TWOSIDED] SkinIndex: 3 Bitmap: HP2HARRY_SKIN01.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: HP2HARRY_SKIN05.bmp  Path: C:\hp2_characters\hp2_harry 
// Original material [5] is [SKIN05.MASKED] SkinIndex: 5 Bitmap: HP2HARRY_SKIN03.bmp  Path: C:\hp2_characters\hp2_harry 
