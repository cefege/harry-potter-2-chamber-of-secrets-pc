//===============================================================================
//  [skProfFlitwick] 
//===============================================================================

class skProfFlitwick extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skProfFlitwickMesh MODELFILE=models\skProfFlitwick.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skProfFlitwickMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skProfFlitwickAnims ANIMFILE=models\skProfFlitwick.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skProfFlitwickMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skProfFlitwickMesh ANIM=skProfFlitwickAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skProfFlitwickAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skProfFlitwickTex0  FILE=TEXTURES\HP2FLITW_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skProfFlitwickTex1  FILE=TEXTURES\HP2FLITW_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skProfFlitwickMesh NUM=0 TEXTURE=skProfFlitwickTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skProfFlitwickMesh NUM=1 TEXTURE=skProfFlitwickTex1

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: HP2FLITW_SKIN00.bmp  Path: C:\potter\Characters\HP2\ProfFlitwick 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2FLITW_SKIN01.bmp  Path: C:\potter\Characters\HP2\ProfFlitwick 
