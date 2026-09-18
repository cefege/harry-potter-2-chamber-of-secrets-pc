//===============================================================================
//  [skForestFern4] 
//===============================================================================

class skForestFern4 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestFern4Mesh MODELFILE=models\skForestFern4.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestFern4Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestFern4Anims ANIMFILE=models\skForestFern4.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestFern4Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestFern4Mesh ANIM=skForestFern4Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestFern4Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestFern4Tex0  FILE=TEXTURES\FernLeaf4.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestFern4Mesh NUM=0 TEXTURE=skForestFern4Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: FernLeaf4.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest\Plants 
