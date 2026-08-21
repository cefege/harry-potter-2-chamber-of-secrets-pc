//===============================================================================
//  [skoilcan] 
//===============================================================================

class skoilcan extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skoilcanMesh MODELFILE=models\skoilcan.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skoilcanMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skoilcanAnims ANIMFILE=models\skoilcan.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skoilcanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skoilcanMesh ANIM=skoilcanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skoilcanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skoilcanTex0  FILE=TEXTURES\HP2OILCAN_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skoilcanMesh NUM=0 TEXTURE=skoilcanTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2OILCAN_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\Oilcan 
