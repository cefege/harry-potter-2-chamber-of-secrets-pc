//===============================================================================
//  [skAragogWeb] 
//===============================================================================

class skAragogWeb extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skAragogWebMesh MODELFILE=models\skAragogWeb.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skAragogWebMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skAragogWebAnims ANIMFILE=models\skAragogWeb.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skAragogWebMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skAragogWebMesh ANIM=skAragogWebAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skAragogWebAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skAragogWebTex0  FILE=TEXTURES\WebSupport.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogWebMesh NUM=0 TEXTURE=skAragogWebTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: WebSupport.bmp  Path: C:\HP2 Art\Textures\Aragog 
