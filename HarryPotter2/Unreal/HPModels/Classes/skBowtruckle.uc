//===============================================================================
//  [skBowtruckle] 
//===============================================================================

class skBowtruckle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skBowtruckleMesh MODELFILE=models\skBowtruckle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBowtruckleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBowtruckleAnims ANIMFILE=models\skBowtruckle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBowtruckleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBowtruckleMesh ANIM=skBowtruckleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBowtruckleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBowtruckleTex0  FILE=TEXTURES\HP2BOW_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBowtruckleTex1  FILE=TEXTURES\HP2BOW_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBowtruckleMesh NUM=0 TEXTURE=skBowtruckleTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBowtruckleMesh NUM=1 TEXTURE=skBowtruckleTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2BOW_SKIN00.bmp  Path: C:\hp2_characters\hp2_bowtruckle 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2BOW_SKIN01.bmp  Path: C:\hp2_characters\hp2_bowtruckle 
