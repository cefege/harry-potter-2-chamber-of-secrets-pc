//===============================================================================
//  [skAragogWebBase] 
//===============================================================================

class skAragogWebBase extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skAragogWebBaseMesh MODELFILE=models\skAragogWebBase.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skAragogWebBaseMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skAragogWebBaseAnims ANIMFILE=models\skAragogWebBase.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skAragogWebBaseMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skAragogWebBaseMesh ANIM=skAragogWebBaseAnims
 
// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skAragogWebBaseAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skAragogWebBaseTex0  FILE=TEXTURES\WebBase.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogWebBaseMesh NUM=0 TEXTURE=skAragogWebBaseTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: WebBase.bmp  Path: C:\HP2 Art\Textures\Aragog 
