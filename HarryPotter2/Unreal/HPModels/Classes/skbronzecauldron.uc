//===============================================================================
//  [skbronzecauldron] 
//===============================================================================

class skbronzecauldron extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbronzecauldronMesh MODELFILE=models\skbronzecauldron.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbronzecauldronMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbronzecauldronAnims ANIMFILE=models\skbronzecauldron.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbronzecauldronMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbronzecauldronMesh ANIM=skbronzecauldronAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbronzecauldronAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbronzecauldronTex0  FILE=TEXTURES\flipcouldronrnze_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbronzecauldronMesh NUM=0 TEXTURE=skbronzecauldronTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: flipcouldronrnze_64.bmp  Path: C:\Nathan 
