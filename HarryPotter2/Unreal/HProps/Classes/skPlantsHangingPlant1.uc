//===============================================================================
//  [skPlantsHangingPlant1] 
//===============================================================================

class skPlantsHangingPlant1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsHangingPlant1Mesh MODELFILE=models\skPlantsHangingPlant1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsHangingPlant1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsHangingPlant1Anims ANIMFILE=models\skPlantsHangingPlant1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsHangingPlant1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsHangingPlant1Mesh ANIM=skPlantsHangingPlant1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsHangingPlant1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsHangingPlant1Tex0  FILE=TEXTURES\HangingPlant1.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsHangingPlant1Mesh NUM=0 TEXTURE=skPlantsHangingPlant1Tex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: HangingPlant1.bmp  Path: C:\Harry Potter 2\ART\Objects\Plants_Trees_Plant Pots_Seeds\Plants\Greenhouse Hanging Plant 


defaultproperties
{
    Mesh=skPlantsHangingPlant1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

