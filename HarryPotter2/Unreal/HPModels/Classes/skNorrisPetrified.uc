//===============================================================================
//  [skNorrisPetrified] 
//===============================================================================

class skNorrisPetrified extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skNorrisPetrifiedMesh MODELFILE=models\skNorrisPetrified.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skNorrisPetrifiedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skNorrisPetrifiedAnims ANIMFILE=models\skNorrisPetrified.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skNorrisPetrifiedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skNorrisPetrifiedMesh ANIM=skNorrisPetrifiedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skNorrisPetrifiedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skNorrisPetrifiedTex0  FILE=TEXTURES\NorrisP_Skin00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skNorrisPetrifiedTex1  FILE=TEXTURES\NorrisTorch_128_02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skNorrisPetrifiedMesh NUM=0 TEXTURE=skNorrisPetrifiedTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skNorrisPetrifiedMesh NUM=1 TEXTURE=skNorrisPetrifiedTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: NorrisP_Skin00.bmp  Path: C:\potter\Characters\HP2\MrsNorris 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: NorrisTorch_128_02.bmp  Path: C:\potter\Characters\HP2\MrsNorris 
