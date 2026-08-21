//===============================================================================
//  [skSpiderSmall] 
//===============================================================================

class skSpiderSmall extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSpiderSmallMesh MODELFILE=models\skSpiderSmall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpiderSmallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpiderSmallAnims ANIMFILE=models\skSpiderSmall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpiderSmallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpiderSmallMesh ANIM=skSpiderSmallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpiderSmallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpiderSmallTex0  FILE=TEXTURES\HP2SPIDERS_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpiderSmallMesh NUM=0 TEXTURE=skSpiderSmallTex0

// Original material [0] is [Skin00] SkinIndex: 0 Bitmap: HP2SPIDERS_SKIN00.bmp  Path: C:\potter\Characters\HP2\SpidersSmall 
