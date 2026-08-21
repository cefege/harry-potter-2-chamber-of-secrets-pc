//===============================================================================
//  [skstalactite] 
//===============================================================================

class skstalactite extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skstalactiteMesh MODELFILE=models\skstalactite.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skstalactiteMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skstalactiteAnims ANIMFILE=models\skstalactite.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skstalactiteMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skstalactiteMesh ANIM=skstalactiteAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skstalactiteAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skstalactiteTex0  FILE=TEXTURES\grayrock_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skstalactiteMesh NUM=0 TEXTURE=skstalactiteTex0

// Original material [0] is [STALA_SKIN00] SkinIndex: 0 Bitmap: grayrock_128.bmp  Path: C:\Nathan 

