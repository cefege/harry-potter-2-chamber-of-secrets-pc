//===============================================================================
//  [skAragogAttack] 
//===============================================================================

class skAragogAttack extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skAragogAttackMesh MODELFILE=models\skAragogAttack.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skAragogAttackMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skAragogAttackAnims ANIMFILE=models\skAragogAttack.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skAragogAttackMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skAragogAttackMesh ANIM=skAragogAttackAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skAragogAttackAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skAragogAttackTex0  FILE=TEXTURES\Aragoo.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skAragogAttackMesh NUM=0 TEXTURE=skAragogAttackTex0

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: Aragoo.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_AragogAttack 

