//===============================================================================
//  [skFordAngliaDamaged] 
//===============================================================================

class skFordAngliaDamaged extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFordAngliaDamagedMesh MODELFILE=models\skFordAngliaDamaged.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFordAngliaDamagedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFordAngliaDamagedAnims ANIMFILE=models\skFordAngliaDamaged.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFordAngliaDamagedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFordAngliaDamagedMesh ANIM=skFordAngliaDamagedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFordAngliaDamagedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFordAngliaDamagedTex0  FILE=TEXTURES\Fordang3_256.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFordAngliaDamagedTex1  FILE=TEXTURES\Fordang4_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFordAngliaDamagedMesh NUM=0 TEXTURE=skFordAngliaDamagedTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFordAngliaDamagedMesh NUM=1 TEXTURE=skFordAngliaDamagedTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Fordang3_256.bmp  Path: C:\Harry Potter 2\ART\Objects\Anglia_Ford 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: Fordang4_256.bmp  Path: C:\Harry Potter 2\ART\Objects\Anglia_Ford 


defaultproperties
{
    Mesh=skFordAngliaDamagedMesh
    DrawType=DT_Mesh
    bStatic=False
}

