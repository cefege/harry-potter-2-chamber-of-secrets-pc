//===============================================================================
//  [skbarrel] 
//===============================================================================

class skbarrel extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbarrelMesh MODELFILE=models\skbarrel.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbarrelMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbarrelAnims ANIMFILE=models\skbarrel.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbarrelMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbarrelMesh ANIM=skbarrelAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbarrelAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbarrelTex0  FILE=TEXTURES\barrelrl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbarrelMesh NUM=0 TEXTURE=skbarrelTex0

// Original material [0] is [BARREL_SKIN00] SkinIndex: 0 Bitmap: barrelrl_128.bmp  Path: C:\Nathan 

