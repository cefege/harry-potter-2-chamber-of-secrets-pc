//===============================================================================
//  [skPlantsWindowBox] 
//===============================================================================

class skPlantsWindowBox extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsWindowBoxMesh MODELFILE=models\skPlantsWindowBox.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsWindowBoxMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsWindowBoxAnims ANIMFILE=models\skPlantsWindowBox.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsWindowBoxMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsWindowBoxMesh ANIM=skPlantsWindowBoxAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsWindowBoxAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsWindowBoxTex0  FILE=TEXTURES\PlanterLeaf_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skPlantsWindowBoxTex1  FILE=TEXTURES\Planter_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsWindowBoxMesh NUM=0 TEXTURE=skPlantsWindowBoxTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsWindowBoxMesh NUM=1 TEXTURE=skPlantsWindowBoxTex1

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: PlanterLeaf_64.bmp  Path: C:\Harry Potter 2\ART\Objects\Plants_Trees_Plant Pots_Seeds\Planter 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: Planter_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Plants_Trees_Plant Pots_Seeds\Planter 


defaultproperties
{
    Mesh=skPlantsWindowBoxMesh
    DrawType=DT_Mesh
    bStatic=False
}

