//===============================================================================
//  [skForestFern3] 
//===============================================================================

class skForestFern3 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestFern3Mesh MODELFILE=models\skForestFern3.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestFern3Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestFern3Anims ANIMFILE=models\skForestFern3.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestFern3Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestFern3Mesh ANIM=skForestFern3Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestFern3Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestFern3Tex0  FILE=TEXTURES\FernLeaf3.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestFern3Mesh NUM=0 TEXTURE=skForestFern3Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: FernLeaf3.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest\Plants 
