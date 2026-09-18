//===============================================================================
//  [skOliverWoodSnitch] 
//===============================================================================

class skOliverWoodSnitch extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skOliverWoodSnitchMesh MODELFILE=models\skOliverWoodSnitch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skOliverWoodSnitchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skOliverWoodSnitchAnims ANIMFILE=models\skOliverWoodSnitch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skOliverWoodSnitchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skOliverWoodSnitchMesh ANIM=skOliverWoodSnitchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skOliverWoodSnitchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skOliverWoodSnitchTex0  FILE=TEXTURES\HP2OLIVER_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodSnitchTex1  FILE=TEXTURES\HP2OLIVER_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skOliverWoodSnitchTex2  FILE=TEXTURES\glsnitch_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodSnitchMesh NUM=0 TEXTURE=skOliverWoodSnitchTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodSnitchMesh NUM=1 TEXTURE=skOliverWoodSnitchTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skOliverWoodSnitchMesh NUM=2 TEXTURE=skOliverWoodSnitchTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2OLIVER_SKIN00.bmp  Path: C:\potter\Characters\HP2\OliverWood 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2OLIVER_SKIN01.bmp  Path: C:\potter\Characters\HP2\OliverWood 
// Original material [2] is [Skin02] SkinIndex: 2 Bitmap: glsnitch_64.bmp  Path: C:\potter\Objects\Snitch 
