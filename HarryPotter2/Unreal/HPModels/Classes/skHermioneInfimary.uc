//===============================================================================
//  [skHermioneInfimary] 
//===============================================================================

class skHermioneInfimary extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHermioneInfimaryMesh MODELFILE=models\skHermioneInfimary.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHermioneInfimaryMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHermioneInfimaryAnims ANIMFILE=models\skHermioneInfimary.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHermioneInfimaryMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHermioneInfimaryMesh ANIM=skHermioneInfimaryAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHermioneInfimaryAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHermioneInfimaryTex0  FILE=TEXTURES\HP2HERMIONEP_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHermioneInfimaryTex1  FILE=TEXTURES\HP2HERMIONEP_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHermioneInfimaryTex2  FILE=TEXTURES\HP2HERMIONEP_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHermioneInfimaryTex3  FILE=TEXTURES\InfirmBed_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHermioneInfimaryTex4  FILE=TEXTURES\InfirmBed_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHermioneInfimaryMesh NUM=0 TEXTURE=skHermioneInfimaryTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHermioneInfimaryMesh NUM=1 TEXTURE=skHermioneInfimaryTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skHermioneInfimaryMesh NUM=2 TEXTURE=skHermioneInfimaryTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skHermioneInfimaryMesh NUM=3 TEXTURE=skHermioneInfimaryTex3
#EXEC MESHMAP SETTEXTURE MESHMAP=skHermioneInfimaryMesh NUM=4 TEXTURE=skHermioneInfimaryTex4

// Original material [0] is [HERMIONE_SKIN00] SkinIndex: 0 Bitmap: HP2HERMIONEP_SKIN00.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [1] is [HERMIONE_SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HERMIONEP_SKIN01.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [2] is [HERMIONE_SKIN02] SkinIndex: 2 Bitmap: HP2HERMIONEP_SKIN02.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [3] is [SKIN03.MASKED] SkinIndex: 3 Bitmap: InfirmBed_128.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [4] is [SKIN04] SkinIndex: 4 Bitmap: InfirmBed_128.bmp  Path: C:\potter\Characters\HP2\Hermione 
