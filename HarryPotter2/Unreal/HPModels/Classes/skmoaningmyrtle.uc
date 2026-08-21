//===============================================================================
//  [skmoaningmyrtle] 
//===============================================================================

class skmoaningmyrtle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skmoaningmyrtleMesh MODELFILE=models\skmoaningmyrtle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skmoaningmyrtleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skmoaningmyrtleAnims ANIMFILE=models\skmoaningmyrtle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skmoaningmyrtleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skmoaningmyrtleMesh ANIM=skmoaningmyrtleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST ANIM=skmoaningmyrtleAnims USERAWINFO VERBOSE

#EXEC TEXTURE IMPORT NAME=skmoaningmyrtleTex0  FILE=TEXTURES\HP2MYRTLE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skmoaningmyrtleTex1  FILE=TEXTURES\HP2MYRTLE_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skmoaningmyrtleTex2  FILE=TEXTURES\HP2MYRTLE_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skmoaningmyrtleMesh NUM=0 TEXTURE=skmoaningmyrtleTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skmoaningmyrtleMesh NUM=1 TEXTURE=skmoaningmyrtleTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skmoaningmyrtleMesh NUM=2 TEXTURE=skmoaningmyrtleTex2

// Original material [0] is [MYRTLE_SKIN00] SkinIndex: 0 Bitmap: HP2MYRTLE_SKIN00.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\MOANING MYRTLE 
// Original material [1] is [MYRTLE_SKIN01] SkinIndex: 1 Bitmap: HP2MYRTLE_SKIN01.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\MOANING MYRTLE 
// Original material [2] is [MYRTLE_SKIN02] SkinIndex: 2 Bitmap: HP2MYRTLE_SKIN02.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\MOANING MYRTLE 
