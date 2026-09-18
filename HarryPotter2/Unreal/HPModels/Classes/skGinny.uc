//===============================================================================
//  [skGinny] 
//===============================================================================

class skGinny extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGinnyMesh MODELFILE=models\skginny.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGinnyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skGinnyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGinnyMesh ANIM=skGenFemaleAnims

#EXEC TEXTURE IMPORT NAME=skGinnyTex0  FILE=TEXTURES\HP2GINNY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGinnyTex1  FILE=TEXTURES\HP2GINNY_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyMesh NUM=0 TEXTURE=skGinnyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGinnyMesh NUM=1 TEXTURE=skGinnyTex1

// Original material [0] is [HERMIONE_SKIN00] SkinIndex: 0 Bitmap: HP2GINNY_SKIN00.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
// Original material [1] is [HERMIONE_SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2GINNY_SKIN01.bmp  Path: C:\hp2_characters\HP2_GinnyWeasley 
