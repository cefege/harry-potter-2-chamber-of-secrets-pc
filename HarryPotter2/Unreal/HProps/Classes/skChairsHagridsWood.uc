//===============================================================================
//  [skChairsHagridsWood] 
//===============================================================================

class skChairsHagridsWood extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsHagridsWoodMesh MODELFILE=models\skChairsHagridsWood.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsHagridsWoodMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsHagridsWoodAnims ANIMFILE=models\skChairsHagridsWood.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsHagridsWoodMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsHagridsWoodMesh ANIM=skChairsHagridsWoodAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsHagridsWoodAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsHagridsWoodTex0  FILE=TEXTURES\hagchar2_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skChairsHagridsWoodTex1  FILE=TEXTURES\hagchar2_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsHagridsWoodMesh NUM=0 TEXTURE=skChairsHagridsWoodTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsHagridsWoodMesh NUM=1 TEXTURE=skChairsHagridsWoodTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: hagchar2_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Hagrids Wood Chair 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: hagchar2_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Hagrids Wood Chair 


defaultproperties
{
    Mesh=skChairsHagridsWoodMesh
    DrawType=DT_Mesh
    bStatic=False
}

