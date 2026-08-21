//===============================================================================
//  [skForestFern2] 
//===============================================================================

class skForestFern2 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestFern2Mesh MODELFILE=models\skForestFern2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestFern2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestFern2Anims ANIMFILE=models\skForestFern2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestFern2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestFern2Mesh ANIM=skForestFern2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestFern2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestFern2Tex0  FILE=TEXTURES\FernLeaf2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestFern2Mesh NUM=0 TEXTURE=skForestFern2Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: FernLeaf2.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest\Plants 
