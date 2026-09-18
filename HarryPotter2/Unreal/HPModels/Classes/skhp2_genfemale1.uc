//===============================================================================
//===============================================================================

class skhp2_genfemale1 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skhp2_genfemale1Mesh MODELFILE=models\skhp2_genfemale1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhp2_genfemale1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skhp2_genfemale1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhp2_genfemale1Mesh ANIM=skGenFemaleAnims

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_0Tex0  FILE=TEXTURES\GFemGRY1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_0Tex1  FILE=TEXTURES\GFemGRY1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_1Tex0  FILE=TEXTURES\GFemGRY2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_1Tex1  FILE=TEXTURES\GFemGRY2_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_2Tex0  FILE=TEXTURES\GFemSLY1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_2Tex1  FILE=TEXTURES\GFemSLY1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_3Tex0  FILE=TEXTURES\GFemSLY2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_3Tex1  FILE=TEXTURES\GFemSLY2_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_4Tex0  FILE=TEXTURES\GFemHUF1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_4Tex1  FILE=TEXTURES\GFemHUF1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_5Tex0  FILE=TEXTURES\GFemHUF2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_5Tex1  FILE=TEXTURES\GFemHUF1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_6Tex0  FILE=TEXTURES\GFemRAV1_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_6Tex1  FILE=TEXTURES\GFemRAV1_SKIN01.bmp  GROUP=Skins

#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_7Tex0  FILE=TEXTURES\GFemRAV2_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhp2_genfemale1_7Tex1  FILE=TEXTURES\GFemRAV1_SKIN01.bmp  GROUP=Skins


//#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genmale1Mesh NUM=0 TEXTURE=skhp2_genmale1Tex0

#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genfemale1Mesh NUM=0 TEXTURE=skhp2_genfemale1_0Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genfemale1Mesh NUM=1 TEXTURE=skhp2_genfemale1_0Tex1

#exec MESH WEAPONATTACH MESH=skhp2_genfemale1Mesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skhp2_genfemale1Mesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [Material #52] SkinIndex: 0 Bitmap: hp2_boyblue.bmp  Path: C:\hp2_characters\hp2_gen_males 
