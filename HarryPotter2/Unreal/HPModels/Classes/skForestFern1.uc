//===============================================================================
//  [skForestFern1] 
//===============================================================================

class skForestFern1 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestFern1Mesh MODELFILE=models\skForestFern1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestFern1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestFern1Anims ANIMFILE=models\skForestFern1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestFern1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestFern1Mesh ANIM=skForestFern1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestFern1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestFern1Tex0  FILE=TEXTURES\FernLeaf1.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestFern1Mesh NUM=0 TEXTURE=skForestFern1Tex0

// Original material [0] is [Fern Leaf] SkinIndex: 0 Bitmap: FernLeaf1.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest 
