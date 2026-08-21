//===============================================================================
//  [skfatfriar] 
//===============================================================================

class skfatfriar extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skfatfriarMesh MODELFILE=models\skfatfriar.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skfatfriarMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skfatfriarAnims ANIMFILE=models\skfatfriar.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skfatfriarMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skfatfriarMesh ANIM=skfatfriarAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skfatfriarAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skfatfriarTex0  FILE=TEXTURES\HP2FRIAR_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skfatfriarTex1  FILE=TEXTURES\HP2FRIAR_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skfatfriarMesh NUM=0 TEXTURE=skfatfriarTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skfatfriarMesh NUM=1 TEXTURE=skfatfriarTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2FRIAR_SKIN00.bmp  Path: C:\hp2_characters\HP2_FatFriar 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2FRIAR_SKIN01.bmp  Path: C:\hp2_characters\HP2_FatFriar 
