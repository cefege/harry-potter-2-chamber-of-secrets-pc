//===============================================================================
//  [skPlantsCactus1] 
//===============================================================================

class skPlantsCactus1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsCactus1Mesh MODELFILE=models\skPlantsCactus1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsCactus1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsCactus1Anims ANIMFILE=models\skPlantsCactus1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsCactus1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsCactus1Mesh ANIM=skPlantsCactus1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsCactus1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsCactus1Tex0  FILE=TEXTURES\Cactus.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsCactus1Mesh NUM=0 TEXTURE=skPlantsCactus1Tex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: Cactus.bmp  Path: C:\Harry Potter\ART\Objects\Plants_Trees_Plant Pots_Seeds\Plants\Greenhouse Cactus 


defaultproperties
{
    Mesh=skPlantsCactus1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

