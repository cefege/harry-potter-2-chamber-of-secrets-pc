//===============================================================================
//  [skAragogStickyWeb] 
//===============================================================================

class skAragogStickyWeb extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skAragogStickyWebMesh MODELFILE=models\skAragogStickyWeb.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skAragogStickyWebMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skAragogStickyWebAnims ANIMFILE=models\skAragogStickyWeb.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skAragogStickyWebMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skAragogStickyWebMesh ANIM=skAragogStickyWebAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skAragogStickyWebAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skAragogStickyWebTex0  FILE=TEXTURES\AragogStickyWeb.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogStickyWebMesh NUM=0 TEXTURE=skAragogStickyWebTex0

// Original material [0] is [SKINOO.TRANSLUCENT] SkinIndex: 0 Bitmap: AragogStickyWeb.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_AragogAttack 
