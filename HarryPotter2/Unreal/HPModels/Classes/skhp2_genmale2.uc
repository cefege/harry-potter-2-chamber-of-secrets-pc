//===============================================================================
//  [skhp2_genmale2] 
//===============================================================================

class skhp2_genmale2 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skhp2_genmale2Mesh MODELFILE=models\skhp2_genmale2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhp2_genmale2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skhp2_genmale2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhp2_genmale2Mesh ANIM=skGenMaleAnims

//#EXEC TEXTURE IMPORT NAME=skhp2_genmale2Tex0  FILE=TEXTURES\hp2_boygreen.bmp  GROUP=Skin
//#EXEC MESHMAP SETTEXTURE MESHMAP=skhp2_genmale2Mesh NUM=0 TEXTURE=skhp2_genmale2Tex0

#exec MESH WEAPONATTACH MESH=skhp2_genmale2Mesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skhp2_genmale2Mesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [GenMale_2] SkinIndex: 0 Bitmap: hp2_boygreen.bmp  Path: C:\hp2_characters\hp2_gen_males 
