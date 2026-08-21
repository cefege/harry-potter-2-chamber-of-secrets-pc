//===============================================================================
//  [skdecanter] 
//===============================================================================

class skdecanter extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skdecanterMesh MODELFILE=models\skdecanter.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skdecanterMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skdecanterAnims ANIMFILE=models\skdecanter.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skdecanterMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skdecanterMesh ANIM=skdecanterAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skdecanterAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skdecanterTex0  FILE=TEXTURES\HP2DECANTB_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdecanterTex1  FILE=TEXTURES\HP2DECANTT_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skdecanterMesh NUM=0 TEXTURE=skdecanterTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skdecanterMesh NUM=1 TEXTURE=skdecanterTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2DECANTB_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\Decanter 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2DECANTT_SKIN01.bmp  Path: C:\potter\Objects\FlipendoObjects\Decanter 
