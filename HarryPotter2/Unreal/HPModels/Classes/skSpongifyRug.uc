//===============================================================================
//  [skSpongifyRug] 
//===============================================================================

class skSpongifyRug extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSpongifyRugMesh MODELFILE=models\skSpongifyRug.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpongifyRugMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpongifyRugAnims ANIMFILE=models\skSpongifyRug.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpongifyRugMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpongifyRugMesh ANIM=skSpongifyRugAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpongifyRugAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpongifyRugTex0  FILE=TEXTURES\SpongeRug.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpongifyRugMesh NUM=0 TEXTURE=skSpongifyRugTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: SpongeRug.bmp  Path: C:\potter\Objects\SpongifyRug 
