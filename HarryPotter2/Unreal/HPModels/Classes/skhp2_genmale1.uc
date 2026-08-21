//===============================================================================
//  [skhp2_genmale1] 
//===============================================================================

class skhp2_genmale1 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skhp2_genmale1Mesh MODELFILE=models\skhp2_genmale1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhp2_genmale1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skhp2_genmale1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhp2_genmale1Mesh ANIM=skGenMaleAnims

//#EXEC TEXTURE IMPORT NAME=skhp2_genmale1Tex0  FILE=TEXTURES\hp2_boyblue.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_0Tex0  FILE=TEXTURES\GOldMaleGRY1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_0Tex1  FILE=TEXTURES\GOldMaleGRY1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_1Tex0  FILE=TEXTURES\GMaleGRY2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_1Tex1  FILE=TEXTURES\GMaleGRY2_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_2Tex0  FILE=TEXTURES\GOldMaleSLY1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_2Tex1  FILE=TEXTURES\GOldMaleSLY1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_3Tex0  FILE=TEXTURES\GMaleSLY2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_3Tex1  FILE=TEXTURES\GMaleSLY2_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_4Tex0  FILE=TEXTURES\GOldMaleHUF1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_4Tex1  FILE=TEXTURES\GOldMaleHUF1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_5Tex0  FILE=TEXTURES\GMaleHUF2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_5Tex1  FILE=TEXTURES\GOldMaleHUF1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_6Tex0  FILE=TEXTURES\GOldMaleRAV1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_6Tex1  FILE=TEXTURES\GOldMaleRAV1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_7Tex0  FILE=TEXTURES\GMaleRAV2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genmale1_7Tex1  FILE=TEXTURES\GOldMaleRAV1_SKIN01.bmp  GROUP=Skins



//#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genmale1Mesh NUM=0 TEXTURE=skhp2_genmale1Tex0

#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genmale1Mesh NUM=0 TEXTURE=skhp2_genmale1_0Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genmale1Mesh NUM=1 TEXTURE=skhp2_genmale1_0Tex1

#exec MESH WEAPONATTACH MESH=skhp2_genmale1Mesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skhp2_genmale1Mesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [Material #52] SkinIndex: 0 Bitmap: hp2_boyblue.bmp  Path: C:\hp2_characters\hp2_gen_males 
