//===============================================================================
//  [skPlantsFlowerRed] 
//===============================================================================

class skPlantsFlowerRed extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsFlowerRedMesh MODELFILE=models\skPlantsFlowerRed.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsFlowerRedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsFlowerRedAnims ANIMFILE=models\skPlantsFlowerRed.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsFlowerRedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsFlowerRedMesh ANIM=skPlantsFlowerRedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsFlowerRedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsFlowerRedTex0  FILE=TEXTURES\GreenHousePlant02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsFlowerRedMesh NUM=0 TEXTURE=skPlantsFlowerRedTex0

// Original material [0] is [skin00.TWOSIDED] SkinIndex: 0 Bitmap: GreenHousePlant02.bmp  Path: C:\Harry Potter\ART\Objects\Plants_Trees_Plant Pots_Seeds\Plants\Greenhouse Plant Red Flower 


defaultproperties
{
    Mesh=skPlantsFlowerRedMesh
    DrawType=DT_Mesh
    bStatic=False
}

