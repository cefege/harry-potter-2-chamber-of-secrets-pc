//===============================================================================
//  [skProfSnape] 
//===============================================================================

class skProfSnape extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skProfSnapeMesh MODELFILE=models\skProfSnape.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skProfSnapeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skProfSnapeAnims ANIMFILE=models\skProfSnape.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skProfSnapeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skProfSnapeMesh ANIM=skProfSnapeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skProfSnapeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skProfSnapeTex0  FILE=TEXTURES\HP2SNAPE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfSnapeTex1  FILE=TEXTURES\HP2SNAPE_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skProfSnapeMesh NUM=0 TEXTURE=skProfSnapeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfSnapeMesh NUM=1 TEXTURE=skProfSnapeTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SNAPE_SKIN00.bmp  Path: C:\potter\Characters\HP2\Snape 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2SNAPE_SKIN01.bmp  Path: C:\potter\Characters\HP2\Snape 
