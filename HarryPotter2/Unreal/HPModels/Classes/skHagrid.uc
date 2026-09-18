//===============================================================================
//  [skHagrid] 
//===============================================================================

class skHagrid extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHagridMesh MODELFILE=models\skHagrid.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHagridMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHagridAnims ANIMFILE=models\skHagrid.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHagridMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHagridMesh ANIM=skHagridAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHagridAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHagridTex0  FILE=TEXTURES\HP2HAGRID_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHagridTex1  FILE=TEXTURES\HP2HAGRID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHagridMesh NUM=0 TEXTURE=skHagridTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHagridMesh NUM=1 TEXTURE=skHagridTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HAGRID_SKIN00.bmp  Path: C:\potter\Characters\HP2\Hagrid 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HAGRID_SKIN01.bmp  Path: C:\potter\Characters\HP2\Hagrid 
