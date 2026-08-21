//===============================================================================
//  [skBeater] 
//===============================================================================

class skBeater extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skBeaterMesh MODELFILE=models\skBeater.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBeaterMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBeaterAnims ANIMFILE=models\skBeater.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBeaterMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBeaterMesh ANIM=skBeaterAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBeaterAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBeater_GTex0  FILE=TEXTURES\MBEATER1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeater_STex0  FILE=TEXTURES\MBEATER2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeater_RTex0  FILE=TEXTURES\MBEATER3_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeater_HTex0  FILE=TEXTURES\MBEATER4_SKIN00.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skBeater_Tex1  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeater_Tex2	 FILE=TEXTURES\MBEATER_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBeaterMesh NUM=0 TEXTURE=skBeater_GTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBeaterMesh NUM=1 TEXTURE=skBeater_Tex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skBeaterMesh NUM=2 TEXTURE=skBeater_Tex2

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: MBEATER1_SKIN00.bmp  Path: C:\~Work\Harry Potter\Characters\MBeater 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: QUID_SKIN01.bmp  Path: C:\~Work\Harry Potter\Characters\MBeater 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: MBEATER_SKIN02.bmp  Path: C:\~Work\Harry Potter\Characters\MBeater 

