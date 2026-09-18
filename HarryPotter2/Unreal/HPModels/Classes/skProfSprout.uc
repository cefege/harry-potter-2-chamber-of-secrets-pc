//===============================================================================
//  [skProfSprout] 
//===============================================================================

class skProfSprout extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skProfSproutMesh MODELFILE=models\skProfSprout.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skProfSproutMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skProfSproutAnims ANIMFILE=models\skProfSprout.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skProfSproutMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skProfSproutMesh ANIM=skProfSproutAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skProfSproutAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skProfSproutTex0  FILE=TEXTURES\HP2SPROUT_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfSproutTex1  FILE=TEXTURES\HP2SPROUT_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skProfSproutMesh NUM=0 TEXTURE=skProfSproutTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfSproutMesh NUM=1 TEXTURE=skProfSproutTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SPROUT_SKIN00.bmp  Path: C:\potter\Characters\HP2\ProfSprout 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2SPROUT_SKIN01.bmp  Path: C:\potter\Characters\HP2\ProfSprout 
