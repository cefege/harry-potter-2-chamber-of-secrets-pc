//===============================================================================
//  [skToolsWateringCan] 
//===============================================================================

class skToolsWateringCan extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skToolsWateringCanMesh MODELFILE=models\skToolsWateringCan.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skToolsWateringCanMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skToolsWateringCanAnims ANIMFILE=models\skToolsWateringCan.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skToolsWateringCanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skToolsWateringCanMesh ANIM=skToolsWateringCanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skToolsWateringCanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skToolsWateringCanTex0  FILE=TEXTURES\GreenHouseWaterCan.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skToolsWateringCanMesh NUM=0 TEXTURE=skToolsWateringCanTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: GreenHouseWaterCan.bmp  Path: C:\Harry Potter\ART\Objects\Tools\Watering Can 


defaultproperties
{
    Mesh=skToolsWateringCanMesh
    DrawType=DT_Mesh
    bStatic=False
}

