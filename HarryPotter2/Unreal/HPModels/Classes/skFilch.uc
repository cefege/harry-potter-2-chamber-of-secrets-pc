//===============================================================================
//  [skFilch] 
//===============================================================================

class skFilch extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skFilchMesh MODELFILE=models\skFilch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFilchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFilchAnims ANIMFILE=models\skFilch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFilchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFilchMesh ANIM=skFilchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFilchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFilchTex0  FILE=TEXTURES\FILCH_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFilchTex1  FILE=TEXTURES\FILCH_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFilchTex2  FILE=TEXTURES\FILCH_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFilchTex3  FILE=TEXTURES\Filtchbr_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFilchMesh NUM=0 TEXTURE=skFilchTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFilchMesh NUM=1 TEXTURE=skFilchTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skFilchMesh NUM=2 TEXTURE=skFilchTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skFilchMesh NUM=3 TEXTURE=skFilchTex3

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: FILCH_SKIN00.bmp  Path: C:\hp2_characters\HP2_Filch 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: FILCH_SKIN01.bmp  Path: C:\hp2_characters\HP2_Filch 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: FILCH_SKIN02.bmp  Path: C:\hp2_characters\HP2_Filch 
// Original material [3] is [SKIN03] SkinIndex: 3 Bitmap: Filtchbr_128.bmp  Path: \\Baker\HPotterPC\Art\Models\Objects\Hogwarts Props\Brooms\Flitchs Broom 
