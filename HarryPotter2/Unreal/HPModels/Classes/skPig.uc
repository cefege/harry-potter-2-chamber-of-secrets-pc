//===============================================================================
//  [skPig] 
//===============================================================================

class skPig extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skPigMesh MODELFILE=models\skPig.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPigMesh X=10 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPigAnims ANIMFILE=models\skPig.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPigMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPigMesh ANIM=skPigAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPigAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPigTex0  FILE=TEXTURES\PIG_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPigMesh NUM=0 TEXTURE=skPigTex0

// Original material [0] is [PIG_SKIN00] SkinIndex: 0 Bitmap: PIG_SKIN00.bmp  Path: C:\hp2_characters\HP2_Pigs 
