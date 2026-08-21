//===============================================================================
//  [skcigarbox] 
//===============================================================================

class skcigarbox extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skcigarboxMesh MODELFILE=models\skcigarbox.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skcigarboxMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skcigarboxAnims ANIMFILE=models\skcigarbox.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skcigarboxMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skcigarboxMesh ANIM=skcigarboxAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skcigarboxAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skcigarboxTex0  FILE=TEXTURES\HP2CIGARB_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skcigarboxTex1  FILE=TEXTURES\HP2CIGART_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skcigarboxMesh NUM=0 TEXTURE=skcigarboxTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skcigarboxMesh NUM=1 TEXTURE=skcigarboxTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2CIGARB_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\CigarBox 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2CIGART_SKIN01.bmp  Path: C:\potter\Objects\FlipendoObjects\CigarBox 
