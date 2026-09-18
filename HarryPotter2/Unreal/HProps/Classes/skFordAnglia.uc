//===============================================================================
//  [skFordAnglia] 
//===============================================================================

class skFordAnglia extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFordAngliaMesh MODELFILE=models\skFordAnglia.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFordAngliaMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFordAngliaAnims ANIMFILE=models\skFordAnglia.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFordAngliaMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFordAngliaMesh ANIM=skFordAngliaAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFordAngliaAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFordAngliaTex0  FILE=TEXTURES\Fordang1_256.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFordAngliaTex1  FILE=TEXTURES\Fordang2_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFordAngliaMesh NUM=0 TEXTURE=skFordAngliaTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFordAngliaMesh NUM=1 TEXTURE=skFordAngliaTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Fordang1_256.bmp  Path: C:\Harry Potter 2\ART\Objects\Anglia_Ford 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: Fordang2_256.bmp  Path: C:\Harry Potter 2\ART\Objects\Anglia_Ford 


defaultproperties
{
    Mesh=skFordAngliaMesh
    DrawType=DT_Mesh
    bStatic=False
}

