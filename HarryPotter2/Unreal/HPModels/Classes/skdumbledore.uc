//===============================================================================
//  [skdumbledore] 
//===============================================================================

class skdumbledore extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skdumbledoreMesh MODELFILE=models\skdumbledore.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skdumbledoreMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skdumbledoreAnims ANIMFILE=models\skdumbledore.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skdumbledoreMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skdumbledoreMesh ANIM=skdumbledoreAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skdumbledoreAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skdumbledoreTex0  FILE=TEXTURES\DUMB_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdumbledoreTex1  FILE=TEXTURES\DUMB_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdumbledoreTex2  FILE=TEXTURES\DUMB_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdumbledoreTex3  FILE=TEXTURES\DUMB_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skdumbledoreMesh NUM=0 TEXTURE=skdumbledoreTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skdumbledoreMesh NUM=1 TEXTURE=skdumbledoreTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skdumbledoreMesh NUM=2 TEXTURE=skdumbledoreTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skdumbledoreMesh NUM=3 TEXTURE=skdumbledoreTex3

#exec ANIM NOTIFY   ANIM=skdumbledoreAnims SEQ=Walk TIME=0.99 FUNCTION=PlayFootStep
#exec ANIM NOTIFY   ANIM=skdumbledoreAnims SEQ=Walk TIME=0.5 FUNCTION=PlayFootStep

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: DUMB_SKIN00.bmp  Path: C:\~Work\Harry Potter\Characters\Dumbledore 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: DUMB_SKIN01.bmp  Path: C:\~Work\Harry Potter\Characters\Dumbledore 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: DUMB_SKIN02.bmp  Path: C:\~Work\Harry Potter\Characters\Dumbledore 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: DUMB_SKIN03.bmp  Path: C:\~Work\Harry Potter\Characters\Dumbledore 

