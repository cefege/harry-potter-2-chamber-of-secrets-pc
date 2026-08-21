//===============================================================================
//  [skPlantsFlowerYellow] 
//===============================================================================

class skPlantsFlowerYellow extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsFlowerYellowMesh MODELFILE=models\skPlantsFlowerYellow.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsFlowerYellowMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsFlowerYellowAnims ANIMFILE=models\skPlantsFlowerYellow.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsFlowerYellowMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsFlowerYellowMesh ANIM=skPlantsFlowerYellowAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsFlowerYellowAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsFlowerYellowTex0  FILE=TEXTURES\Flower2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsFlowerYellowMesh NUM=0 TEXTURE=skPlantsFlowerYellowTex0

// Original material [0] is [skin00.MASKED] SkinIndex: 0 Bitmap: Flower2.bmp  Path: C:\Harry Potter\ART\Objects\Plants_Trees_Plant Pots_Seeds\Plants\Yellow Flower 


defaultproperties
{
    Mesh=skPlantsFlowerYellowMesh
    DrawType=DT_Mesh
    bStatic=False
}

