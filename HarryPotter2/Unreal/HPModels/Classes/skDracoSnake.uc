//===============================================================================
//  [skdracosnake] 
//===============================================================================

class skdracosnake extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skdracosnakeMesh MODELFILE=models\skdracosnake.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skdracosnakeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skdracosnakeAnims ANIMFILE=models\skdracosnake.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skdracosnakeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skdracosnakeMesh ANIM=skdracosnakeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skdracosnakeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skdracosnakeTex0  FILE=TEXTURES\HP2DRACO_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdracosnakeTex1  FILE=TEXTURES\HP2DRACO_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skdracosnakeMesh NUM=0 TEXTURE=skdracosnakeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skdracosnakeMesh NUM=1 TEXTURE=skdracosnakeTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2DRACO_SKIN00.bmp  Path: C:\hp2_characters\HP2_DracoMalfoy 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2DRACO_SKIN01.bmp  Path: C:\hp2_characters\HP2_DracoMalfoy 
