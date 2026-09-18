//===============================================================================
//  [skfirecrab] 
//===============================================================================

class skfirecrab extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skfirecrabMesh MODELFILE=models\skfirecrab.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skfirecrabMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skfirecrabAnims ANIMFILE=models\skfirecrab.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skfirecrabMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skfirecrabMesh ANIM=skfirecrabAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skfirecrabAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skfirecrabTex0  FILE=TEXTURES\HP2Firecrab_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skfirecrabTex1  FILE=TEXTURES\HP2Firecrab_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skfirecrabMesh NUM=0 TEXTURE=skfirecrabTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skfirecrabMesh NUM=1 TEXTURE=skfirecrabTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2Firecrab_SKIN00.bmp  Path: C:\~Work\Harry Potter\HP2 Characters\HP2_Firecrab 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2Firecrab_SKIN01.bmp  Path: C:\~Work\Harry Potter\HP2 Characters\HP2_Firecrab 
