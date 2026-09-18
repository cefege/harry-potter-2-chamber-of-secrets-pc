//===============================================================================
//  [skjewelbox] 
//===============================================================================

class skjewelbox extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skjewelboxMesh MODELFILE=models\skjewelbox.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skjewelboxMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skjewelboxAnims ANIMFILE=models\skjewelbox.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skjewelboxMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skjewelboxMesh ANIM=skjewelboxAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skjewelboxAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skjewelboxTex0  FILE=TEXTURES\HP2JEWELB_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skjewelboxTex1  FILE=TEXTURES\HP2JEWELB_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skjewelboxMesh NUM=0 TEXTURE=skjewelboxTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skjewelboxMesh NUM=1 TEXTURE=skjewelboxTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2JEWELB_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\JewelBox 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2JEWELB_SKIN01.bmp  Path: C:\potter\Objects\FlipendoObjects\JewelBox 
