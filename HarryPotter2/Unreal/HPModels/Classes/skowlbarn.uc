//===============================================================================
//  [skowlbarn] 
//===============================================================================

class skowlbarn extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skowlbarnMesh MODELFILE=models\skowlbarn.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skowlbarnMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skowlbarnAnims ANIMFILE=models\skowlbarn.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skowlbarnMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skowlbarnMesh ANIM=skowlbarnAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skowlbarnAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skowlbarnTex0  FILE=TEXTURES\Hedwig_skin00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skowlbarnMesh NUM=0 TEXTURE=skowlbarnTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Hedwig_skin00.bmp  Path: C:\POTTER\Art\Characters\Hedwig 

