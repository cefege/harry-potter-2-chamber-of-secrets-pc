//===============================================================================
//  [skmusicbox] 
//===============================================================================

class skmusicbox extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skmusicboxMesh MODELFILE=models\skmusicbox.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skmusicboxMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skmusicboxAnims ANIMFILE=models\skmusicbox.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skmusicboxMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skmusicboxMesh ANIM=skmusicboxAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skmusicboxAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skmusicboxTex0  FILE=TEXTURES\HP2MUSICB_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skmusicboxTex1  FILE=TEXTURES\HP2MUSICB_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skmusicboxMesh NUM=0 TEXTURE=skmusicboxTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skmusicboxMesh NUM=1 TEXTURE=skmusicboxTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2MUSICB_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\MusicBox 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2MUSICB_SKIN01.bmp  Path: C:\potter\Objects\FlipendoObjects\MusicBox 
