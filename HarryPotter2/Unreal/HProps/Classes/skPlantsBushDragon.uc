//===============================================================================
//  [skPlantsBushDragon] 
//===============================================================================

class skPlantsBushDragon extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsBushDragonMesh MODELFILE=models\skPlantsBushDragon.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsBushDragonMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsBushDragonAnims ANIMFILE=models\skPlantsBushDragon.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsBushDragonMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsBushDragonMesh ANIM=skPlantsBushDragonAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsBushDragonAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsBushDragonTex0  FILE=TEXTURES\DragonBush.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsBushDragonMesh NUM=0 TEXTURE=skPlantsBushDragonTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: DragonBush.bmp  Path: C:\Harry Potter\ART\Objects\Plants_Trees_Plant Pots_Seeds\Bushes\Dragon Bush 


defaultproperties
{
    Mesh=skPlantsBushDragonMesh
    DrawType=DT_Mesh
    bStatic=False
}

