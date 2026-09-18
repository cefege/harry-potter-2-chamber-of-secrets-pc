
//===============================================================================
//  [skbasilisk] 
//===============================================================================

class skbasilisk extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbasiliskMesh MODELFILE=models\skbasilisk.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbasiliskMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbasiliskAnims ANIMFILE=models\skbasilisk.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbasiliskMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbasiliskMesh ANIM=skbasiliskAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbasiliskAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbasiliskTex0  FILE=TEXTURES\HP2BASILISK_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskTex1  FILE=TEXTURES\HP2BASILISK_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskTex2  FILE=TEXTURES\HP2BASILISK_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasiliskTex3  FILE=TEXTURES\HP2BASILISK_SKIN04.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskMesh NUM=0 TEXTURE=skbasiliskTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskMesh NUM=1 TEXTURE=skbasiliskTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskMesh NUM=2 TEXTURE=skbasiliskTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasiliskMesh NUM=3 TEXTURE=skbasiliskTex3

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2BASILISK_SKIN00.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2BASILISK_SKIN01.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2BASILISK_SKIN02.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [3] is [SKIN04] SkinIndex: 4 Bitmap: HP2BASILISK_SKIN04.bmp  Path: C:\hp2_characters\HP2_basilisk 


#exec ANIM NOTIFY   ANIM=skbasiliskAnims SEQ=Idle TIME=0.426 FUNCTION=PlayHissSound
#exec ANIM NOTIFY   ANIM=skbasiliskAnims SEQ=Idle TIME=0.787 FUNCTION=PlayHissSound
