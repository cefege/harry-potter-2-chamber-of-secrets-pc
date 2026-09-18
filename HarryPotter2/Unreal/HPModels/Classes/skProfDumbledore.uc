//===============================================================================
//  [skProfDumbledore] 
//===============================================================================

class skProfDumbledore extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skProfDumbledoreMesh MODELFILE=models\skProfDumbledore.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skProfDumbledoreMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skProfDumbledoreAnims ANIMFILE=models\skProfDumbledore.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skProfDumbledoreMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skProfDumbledoreMesh ANIM=skProfDumbledoreAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skProfDumbledoreAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skProfDumbledoreTex0  FILE=TEXTURES\HP2DUMB_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfDumbledoreTex1  FILE=TEXTURES\HP2DUMB_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfDumbledoreTex2  FILE=TEXTURES\HP2DUMB_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfDumbledoreTex3  FILE=TEXTURES\HP2DUMB_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skProfDumbledoreMesh NUM=0 TEXTURE=skProfDumbledoreTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfDumbledoreMesh NUM=1 TEXTURE=skProfDumbledoreTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfDumbledoreMesh NUM=2 TEXTURE=skProfDumbledoreTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfDumbledoreMesh NUM=3 TEXTURE=skProfDumbledoreTex3

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: HP2DUMB_SKIN00.bmp  Path: C:\potter\Characters\HP2\Dumbledore 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2DUMB_SKIN01.bmp  Path: C:\potter\Characters\HP2\Dumbledore 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2DUMB_SKIN02.bmp  Path: C:\potter\Characters\HP2\Dumbledore 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: HP2DUMB_SKIN03.bmp  Path: C:\potter\Characters\HP2\Dumbledore 
