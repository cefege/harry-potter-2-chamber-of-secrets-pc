//===============================================================================
//  [skdobby] 
//===============================================================================

class skdobby extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skdobbyMesh MODELFILE=models\skdobby.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skdobbyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skdobbyAnims ANIMFILE=models\skdobby.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skdobbyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skdobbyMesh ANIM=skdobbyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST ANIM=skdobbyAnims USERAWINFO VERBOSE

#EXEC TEXTURE IMPORT NAME=skdobbyTex0  FILE=TEXTURES\HP2DOBBY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdobbyTex1  FILE=TEXTURES\HP2DOBBY_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skdobbyMesh NUM=0 TEXTURE=skdobbyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skdobbyMesh NUM=1 TEXTURE=skdobbyTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2DOBBY_SKIN00.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\DOBBY 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2DOBBY_SKIN01.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\DOBBY 
