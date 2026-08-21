//===============================================================================
//  [skChairsHagridsLeather] 
//===============================================================================

class skChairsHagridsLeather extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsHagridsLeatherMesh MODELFILE=models\skChairsHagridsLeather.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsHagridsLeatherMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsHagridsLeatherAnims ANIMFILE=models\skChairsHagridsLeather.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsHagridsLeatherMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsHagridsLeatherMesh ANIM=skChairsHagridsLeatherAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsHagridsLeatherAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsHagridsLeatherTex0  FILE=TEXTURES\hagchair_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsHagridsLeatherMesh NUM=0 TEXTURE=skChairsHagridsLeatherTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: hagchair_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Hagrids Leather Chair 


defaultproperties
{
    Mesh=skChairsHagridsLeatherMesh
    DrawType=DT_Mesh
    bStatic=False
}

