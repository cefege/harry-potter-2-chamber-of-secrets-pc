//===============================================================================
//  [skForestFern3b] 
//===============================================================================

class skForestFern3b extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestFern3bMesh MODELFILE=models\skForestFern3b.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestFern3bMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestFern3bAnims ANIMFILE=models\skForestFern3b.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestFern3bMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestFern3bMesh ANIM=skForestFern3bAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestFern3bAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestFern3bTex0  FILE=TEXTURES\FernLeaf3.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestFern3bMesh NUM=0 TEXTURE=skForestFern3bTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: FernLeaf3.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest\Plants 
