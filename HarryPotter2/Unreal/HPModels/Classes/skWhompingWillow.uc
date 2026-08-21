//===============================================================================
//  [skWhompingWIllow] 
//===============================================================================

class skWhompingWIllow extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skWhompingWIllowMesh MODELFILE=models\skWhompingWIllow.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWhompingWIllowMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWhompingWIllowAnims ANIMFILE=models\skWhompingWIllow.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWhompingWIllowMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWhompingWIllowMesh ANIM=skWhompingWIllowAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWhompingWIllowAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skWhompingWIllowTex0  FILE=TEXTURES\HP2_WWFlipendoTarget.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWhompingWIllowTex1  FILE=TEXTURES\HP2_WWBranchTrans.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWhompingWIllowTex2  FILE=TEXTURES\HP2_WWRootBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWhompingWIllowTex3  FILE=TEXTURES\HP2_WWUnderRoot.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWhompingWIllowTex4  FILE=TEXTURES\HP2_WWDooDoo.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompingWIllowMesh NUM=0 TEXTURE=skWhompingWIllowTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompingWIllowMesh NUM=1 TEXTURE=skWhompingWIllowTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompingWIllowMesh NUM=2 TEXTURE=skWhompingWIllowTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompingWIllowMesh NUM=3 TEXTURE=skWhompingWIllowTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompingWIllowMesh NUM=4 TEXTURE=skWhompingWIllowTex4

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2_WWFlipendoTarget.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_WhompingWillow 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2_WWBranchTrans.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_WhompingWillow 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2_WWRootBark.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_WhompingWillow 
// Original material [3] is [SKIN03] SkinIndex: 3 Bitmap: HP2_WWUnderRoot.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_WhompingWillow 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: HP2_WWDooDoo.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\HP2 Characters\HP2_WhompingWillow 
