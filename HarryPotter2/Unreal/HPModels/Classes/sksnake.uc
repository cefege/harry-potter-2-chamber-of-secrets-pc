//===============================================================================
//  [sksnake] 
//===============================================================================

class sksnake extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=sksnakeMesh MODELFILE=models\sksnake.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=sksnakeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=sksnakeAnims ANIMFILE=models\sksnake.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=sksnakeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=sksnakeMesh ANIM=sksnakeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=sksnakeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=sksnakeTex0  FILE=TEXTURES\HP2SNAKE_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=sksnakeMesh NUM=0 TEXTURE=sksnakeTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SNAKE_SKIN00.bmp  Path: C:\potter\Characters\HP2\Snake 
