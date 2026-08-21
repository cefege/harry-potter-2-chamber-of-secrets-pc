//===============================================================================
//  [skHarryHand] 
//===============================================================================

class skHarryHand extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHarryHandMesh MODELFILE=models\skHarryHand.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHarryHandMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHarryHandAnims ANIMFILE=models\skHarryHand.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHarryHandMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHarryHandMesh ANIM=skHarryHandAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHarryHandAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHarryHandTex0  FILE=TEXTURES\HP2HarryHand_skin00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHarryHandMesh NUM=0 TEXTURE=skHarryHandTex0

// Original material [0] is [Arm] SkinIndex: 0 Bitmap: HP2HarryHand_skin00.bmp  Path: C:\hp2_characters\hp2_harry 
