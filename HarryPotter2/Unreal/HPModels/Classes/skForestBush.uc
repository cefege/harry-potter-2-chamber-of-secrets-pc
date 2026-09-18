//===============================================================================
//  [skForestBush] 
//===============================================================================

class skForestBush extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skForestBushMesh MODELFILE=models\skForestBush.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skForestBushMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skForestBushAnims ANIMFILE=models\skForestBush.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skForestBushMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skForestBushMesh ANIM=skForestBushAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skForestBushAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skForestBushTex0  FILE=TEXTURES\ForestBush.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skForestBushMesh NUM=0 TEXTURE=skForestBushTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: ForestBush.bmp  Path: C:\HP2 Art\Textures\ForbiddenForest\Plants 
