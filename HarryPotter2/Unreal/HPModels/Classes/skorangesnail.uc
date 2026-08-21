//===============================================================================
//  [skorangesnail] 
//===============================================================================

class skorangesnail extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skorangesnailMesh MODELFILE=models\skorangesnail.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skorangesnailMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skorangesnailAnims ANIMFILE=models\skorangesnail.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skorangesnailMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skorangesnailMesh ANIM=skorangesnailAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST ANIM=skorangesnailAnims USERAWINFO VERBOSE

#EXEC TEXTURE IMPORT NAME=skorangesnailTex0  FILE=TEXTURES\HP2SNAIL_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skorangesnailMesh NUM=0 TEXTURE=skorangesnailTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: HP2SNAIL_SKIN00.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\ORANGE SNAIL 
