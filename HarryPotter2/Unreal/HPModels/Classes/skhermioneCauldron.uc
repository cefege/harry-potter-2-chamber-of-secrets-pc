//===============================================================================
//  [skhermioneCauldron] 
//===============================================================================

class skhermioneCauldron extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skhermioneCauldronMesh MODELFILE=models\skhermioneCauldron.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhermioneCauldronMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skhermioneCauldronAnims ANIMFILE=models\skhermioneCauldron.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skhermioneCauldronMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhermioneCauldronMesh ANIM=skhermioneCauldronAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skhermioneCauldronAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skhermioneCauldronTex0  FILE=TEXTURES\HP2HERMIONE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneCauldronTex1  FILE=TEXTURES\HP2HERMIONE_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneCauldronTex2  FILE=TEXTURES\HP2HERMIONE_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneCauldronTex3  FILE=TEXTURES\couldron_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneCauldronTex4  FILE=TEXTURES\juiceglass_skin06.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneCauldronTex5  FILE=TEXTURES\HWBeaker_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneCauldronMesh NUM=0 TEXTURE=skhermioneCauldronTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneCauldronMesh NUM=1 TEXTURE=skhermioneCauldronTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneCauldronMesh NUM=2 TEXTURE=skhermioneCauldronTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneCauldronMesh NUM=3 TEXTURE=skhermioneCauldronTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneCauldronMesh NUM=4 TEXTURE=skhermioneCauldronTex4
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneCauldronMesh NUM=5 TEXTURE=skhermioneCauldronTex5

// Original material [0] is [HERMIONE_SKIN00] SkinIndex: 0 Bitmap: HP2HERMIONE_SKIN00.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [1] is [HERMIONE_SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HERMIONE_SKIN01.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [2] is [HERMIONE_SKIN02] SkinIndex: 2 Bitmap: HP2HERMIONE_SKIN02.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [3] is [SKIN03.TWOSIDED] SkinIndex: 3 Bitmap: couldron_64.bmp  Path: C:\potter\Objects\Cauldron 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: juiceglass_skin06.bmp  Path: \\Baker\HPotterPC\Art\AnimDump\glass 
// Original material [5] is [SKIN05.TRANSLUCENT] SkinIndex: 5 Bitmap: HWBeaker_128.bmp  Path: C:\potter\Objects\Jars 
