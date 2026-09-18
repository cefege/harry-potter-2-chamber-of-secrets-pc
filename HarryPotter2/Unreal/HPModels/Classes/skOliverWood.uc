//===============================================================================
//  [skOliverWood] 
//===============================================================================

class skOliverWood extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skOliverWoodMesh MODELFILE=models\skOliverWood.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skOliverWoodMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skOliverWoodMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skOliverWoodMesh ANIM=skGeorgeWeasleyAnims

#EXEC TEXTURE IMPORT NAME=skOliverWoodTex0  FILE=TEXTURES\HP2OLIVER_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodTex1  FILE=TEXTURES\HP2OLIVER_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodMesh NUM=0 TEXTURE=skOliverWoodTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodMesh NUM=1 TEXTURE=skOliverWoodTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2OLIVER_SKIN00.bmp  Path: C:\hp2_characters\HP2_OliverWood 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2OLIVER_SKIN01.bmp  Path: C:\hp2_characters\HP2_OliverWood 
