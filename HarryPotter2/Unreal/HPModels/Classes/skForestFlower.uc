//===============================================================================
//  [skForestFlower] 
//===============================================================================

class skForestFlower extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestFlowerMesh MODELFILE=models\skForestFlower.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestFlowerMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestFlowerAnims ANIMFILE=models\skForestFlower.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestFlowerMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestFlowerMesh ANIM=skForestFlowerAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestFlowerAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestFlowerTex0  FILE=TEXTURES\ForestFlower.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestFlowerMesh NUM=0 TEXTURE=skForestFlowerTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: ForestFlower.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest\Plants 
