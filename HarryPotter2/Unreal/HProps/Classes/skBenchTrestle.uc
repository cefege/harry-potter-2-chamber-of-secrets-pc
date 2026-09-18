//===============================================================================
//  [skBenchTrestle] 
//===============================================================================

class skBenchTrestle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBenchTrestleMesh MODELFILE=models\skBenchTrestle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBenchTrestleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBenchTrestleAnims ANIMFILE=models\skBenchTrestle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBenchTrestleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBenchTrestleMesh ANIM=skBenchTrestleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBenchTrestleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBenchTrestleTex0  FILE=TEXTURES\hogbench_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBenchTrestleMesh NUM=0 TEXTURE=skBenchTrestleTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: hogbench_128.bmp  Path: C:\Harry Potter\ART\Objects\Benches\Tressle Bench 


defaultproperties
{
    Mesh=skBenchTrestleMesh
    DrawType=DT_Mesh
    bStatic=False
}

