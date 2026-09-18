//===============================================================================
//  [skBenchTable] 
//===============================================================================

class skBenchTable extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBenchTableMesh MODELFILE=models\skBenchTable.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBenchTableMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBenchTableAnims ANIMFILE=models\skBenchTable.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBenchTableMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBenchTableMesh ANIM=skBenchTableAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBenchTableAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBenchTableTex0  FILE=TEXTURES\tablbnch_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBenchTableMesh NUM=0 TEXTURE=skBenchTableTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: tablbnch_128.bmp  Path: C:\HP_2\Objects\Tables_Desks\Great Hall TableBench 


defaultproperties
{
    Mesh=skBenchTableMesh
    DrawType=DT_Mesh
    bStatic=False
}

