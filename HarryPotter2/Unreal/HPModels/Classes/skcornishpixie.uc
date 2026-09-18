//===============================================================================
//  [skcornishpixie] 
//===============================================================================

class skcornishpixie extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skcornishpixieMesh MODELFILE=models\skcornishpixie.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skcornishpixieMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
//#exec MESH  ORIGIN MESH=skcornishpixieMesh X=0 Y=0 Z=23 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skcornishpixieAnims ANIMFILE=models\skcornishpixie.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skcornishpixieMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skcornishpixieMesh ANIM=skcornishpixieAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skcornishpixieAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skcornishpixieTex0  FILE=TEXTURES\HP2PIXIE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skcornishpixieTex1  FILE=TEXTURES\HP2PIXIE_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skcornishpixieMesh NUM=0 TEXTURE=skcornishpixieTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skcornishpixieMesh NUM=1 TEXTURE=skcornishpixieTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2PIXIE_SKIN00.bmp  Path: C:\potter\Characters\HP2\CornishPixie 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2PIXIE_SKIN01.bmp  Path: C:\potter\Characters\HP2\CornishPixie 
