//===============================================================================
//  [skimp] 
//===============================================================================

class skimp extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skimpMesh MODELFILE=models\skImp.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skimpMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skimpAnims ANIMFILE=models\skimp.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skimpMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skimpMesh ANIM=skimpAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skimpAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skimpTex0  FILE=TEXTURES\HP2IMP_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skimpMesh NUM=0 TEXTURE=skimpTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2IMP_SKIN00.bmp  Path: C:\hp2_characters\HP2_Imps 
