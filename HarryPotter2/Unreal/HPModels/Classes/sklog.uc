//===============================================================================
//  [sklog] 
//===============================================================================

class sklog extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=sklogMesh MODELFILE=models\sklog.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=sklogMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=sklogAnims ANIMFILE=models\sklog.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=sklogMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=sklogMesh ANIM=sklogAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=sklogAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=sklogTex0  FILE=TEXTURES\flipylog_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=sklogMesh NUM=0 TEXTURE=sklogTex0

// Original material [0] is [LOG_SKIN00] SkinIndex: 0 Bitmap: flipylog_128.bmp  Path: C:\Nathan 

