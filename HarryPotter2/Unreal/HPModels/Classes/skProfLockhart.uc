//===============================================================================
//  [skProfLockhart] 
//===============================================================================

class skProfLockhart extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skProfLockhartMesh MODELFILE=models\skProfLockhart.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skProfLockhartMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skProfLockhartAnims ANIMFILE=models\skProfLockhart.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skProfLockhartMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skProfLockhartMesh ANIM=skProfLockhartAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skProfLockhartAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skProfLockhartTex0  FILE=TEXTURES\HP2LOCK_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfLockhartTex1  FILE=TEXTURES\HP2LOCK_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skProfLockhartMesh NUM=0 TEXTURE=skProfLockhartTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfLockhartMesh NUM=1 TEXTURE=skProfLockhartTex1

// Original material [0] is [skin00] SkinIndex: 0 Bitmap: HP2LOCK_SKIN00.bmp  Path: C:\potter\Characters\HP2\GilderoyLockhart 
// Original material [1] is [skin01] SkinIndex: 1 Bitmap: HP2LOCK_SKIN01.bmp  Path: C:\potter\Characters\HP2\GilderoyLockhart 
