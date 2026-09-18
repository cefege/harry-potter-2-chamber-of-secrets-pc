//===============================================================================
//  [skBenchWithArms] 
//===============================================================================

class skBenchWithArms extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBenchWithArmsMesh MODELFILE=models\skBenchWithArms.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBenchWithArmsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBenchWithArmsAnims ANIMFILE=models\skBenchWithArms.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBenchWithArmsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBenchWithArmsMesh ANIM=skBenchWithArmsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBenchWithArmsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBenchWithArmsTex0  FILE=TEXTURES\grybench_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBenchWithArmsMesh NUM=0 TEXTURE=skBenchWithArmsTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: grybench_128.bmp  Path: C:\Harry Potter\ART\Objects\Benches\Arm Bench 


defaultproperties
{
    Mesh=skBenchWithArmsMesh
    DrawType=DT_Mesh
    bStatic=False
}

