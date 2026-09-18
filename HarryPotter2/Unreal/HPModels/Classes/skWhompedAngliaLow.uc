//===============================================================================
//  [skWhompedAngliaLow] 
//===============================================================================

class skWhompedAngliaLow extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skWhompedAngliaLowMesh MODELFILE=models\skWhompedAngliaLow.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWhompedAngliaLowMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWhompedAngliaLowAnims ANIMFILE=models\skWhompedAngliaLow.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWhompedAngliaLowMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWhompedAngliaLowMesh ANIM=skWhompedAngliaLowAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWhompedAngliaLowAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skWhompedAngliaLowTex0  FILE=TEXTURES\Fordang5_256.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWhompedAngliaLowTex1  FILE=TEXTURES\Fordang4_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompedAngliaLowMesh NUM=0 TEXTURE=skWhompedAngliaLowTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skWhompedAngliaLowMesh NUM=1 TEXTURE=skWhompedAngliaLowTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Fordang5_256.bmp  Path: C:\potter\Characters\HP2\Anglia_Ford 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: Fordang4_256.bmp  Path: C:\potter\Characters\HP2\Anglia_Ford 
