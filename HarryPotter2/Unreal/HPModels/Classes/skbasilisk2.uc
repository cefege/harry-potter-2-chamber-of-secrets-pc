//===============================================================================
//  [skbasilisk2] 
//===============================================================================

class skbasilisk2 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbasilisk2Mesh MODELFILE=models\skbasilisk2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbasilisk2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbasilisk2Anims ANIMFILE=models\skbasilisk2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbasilisk2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbasilisk2Mesh ANIM=skbasilisk2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbasilisk2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbasilisk2Tex0  FILE=TEXTURES\HP2BASILISK_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasilisk2Tex1  FILE=TEXTURES\HP2BASILISK_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasilisk2Tex2  FILE=TEXTURES\HP2BASILISK_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skbasilisk2Tex3  FILE=TEXTURES\HP2BASILISK_SKIN04.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbasilisk2Mesh NUM=0 TEXTURE=skbasilisk2Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasilisk2Mesh NUM=1 TEXTURE=skbasilisk2Tex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasilisk2Mesh NUM=2 TEXTURE=skbasilisk2Tex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skbasilisk2Mesh NUM=3 TEXTURE=skbasilisk2Tex3

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2BASILISK_SKIN00.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2BASILISK_SKIN01.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2BASILISK_SKIN02.bmp  Path: C:\hp2_characters\HP2_basilisk 
// Original material [3] is [SKIN04] SkinIndex: 4 Bitmap: HP2BASILISK_SKIN04.bmp  Path: C:\hp2_characters\HP2_basilisk 
