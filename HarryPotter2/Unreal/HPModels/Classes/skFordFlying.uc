//===============================================================================
//  [skFordFlying] 
//===============================================================================

class skFordFlying extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skFordFlyingMesh MODELFILE=models\skFordFlying.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFordFlyingMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFordFlyingAnims ANIMFILE=models\skFordFlying.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFordFlyingMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFordFlyingMesh ANIM=skFordFlyingAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFordFlyingAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFordFlyingTex0  FILE=TEXTURES\Fordang1_256.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFordFlyingTex1  FILE=TEXTURES\Fordang2_256.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFordFlyingTex2  FILE=TEXTURES\FordangLight.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFordFlyingMesh NUM=0 TEXTURE=skFordFlyingTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFordFlyingMesh NUM=1 TEXTURE=skFordFlyingTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skFordFlyingMesh NUM=2 TEXTURE=skFordFlyingTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Fordang1_256.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_FordAngliaFlying 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: Fordang2_256.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_FordAngliaFlying 
// Original material [2] is [SKIN02.TRANS] SkinIndex: 2 Bitmap: FordangLight.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_FordAngliaFlying 
