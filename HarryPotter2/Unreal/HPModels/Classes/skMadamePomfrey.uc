//===============================================================================
//  [skmadamepomfrey] 
//===============================================================================

class skmadamepomfrey extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skmadamepomfreyMesh MODELFILE=models\skmadamepomfrey.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skmadamepomfreyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skmadamepomfreyAnims ANIMFILE=models\skmadamepomfrey.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skmadamepomfreyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skmadamepomfreyMesh ANIM=skmadamepomfreyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST ANIM=skmadamepomfreyAnims USERAWINFO VERBOSE

#EXEC TEXTURE IMPORT NAME=skmadamepomfreyTex0  FILE=TEXTURES\HP2POMP_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skmadamepomfreyTex1  FILE=TEXTURES\HP2POMP_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skmadamepomfreyMesh NUM=0 TEXTURE=skmadamepomfreyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skmadamepomfreyMesh NUM=1 TEXTURE=skmadamepomfreyTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2POMP_SKIN00.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\MADAME POMFREY 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2POMP_SKIN01.bmp  Path: C:\Documents and Settings\Nathan Hocken\My Documents\HARRYPOTTER2\PRODUCTION\MADAME POMFREY 
