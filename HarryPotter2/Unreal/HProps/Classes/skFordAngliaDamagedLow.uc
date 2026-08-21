//===============================================================================
//  [skFordAngliaDamagedLow] 
//===============================================================================

class skFordAngliaDamagedLow extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFordAngliaDamagedLowMesh MODELFILE=models\skFordAngliaDamagedLow.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFordAngliaDamagedLowMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFordAngliaDamagedLowAnims ANIMFILE=models\skFordAngliaDamagedLow.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFordAngliaDamagedLowMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFordAngliaDamagedLowMesh ANIM=skFordAngliaDamagedLowAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFordAngliaDamagedLowAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFordAngliaDamagedLowTex0  FILE=TEXTURES\Fordang5_256.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFordAngliaDamagedLowTex1  FILE=TEXTURES\Fordang4_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFordAngliaDamagedLowMesh NUM=0 TEXTURE=skFordAngliaDamagedLowTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFordAngliaDamagedLowMesh NUM=1 TEXTURE=skFordAngliaDamagedLowTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Fordang5_256.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\ANGLIA_FORD 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: Fordang4_256.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\ANGLIA_FORD 


defaultproperties
{
    Mesh=skFordAngliaDamagedLowMesh
    DrawType=DT_Mesh
    bStatic=False
}

