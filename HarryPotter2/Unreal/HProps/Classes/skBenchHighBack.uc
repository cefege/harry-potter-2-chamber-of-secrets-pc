//===============================================================================
//  [skBenchHighBack] 
//===============================================================================

class skBenchHighBack extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBenchHighBackMesh MODELFILE=models\skBenchHighBack.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBenchHighBackMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBenchHighBackAnims ANIMFILE=models\skBenchHighBack.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBenchHighBackMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBenchHighBackMesh ANIM=skBenchHighBackAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBenchHighBackAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBenchHighBackTex0  FILE=TEXTURES\HWbenchT_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBenchHighBackMesh NUM=0 TEXTURE=skBenchHighBackTex0

// Original material [0] is [Material #7] SkinIndex: 0 Bitmap: HWbenchT_128.bmp  Path: C:\Harry Potter\ART\Objects\Benches\High Backed Bench 


defaultproperties
{
    Mesh=skBenchHighBackMesh
    DrawType=DT_Mesh
    bStatic=False
}

