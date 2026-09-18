//===============================================================================
//  [skBathroomToiletTank] 
//===============================================================================

class skBathroomToiletTank extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBathroomToiletTankMesh MODELFILE=models\skBathroomToiletTank.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBathroomToiletTankMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBathroomToiletTankAnims ANIMFILE=models\skBathroomToiletTank.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBathroomToiletTankMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBathroomToiletTankMesh ANIM=skBathroomToiletTankAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBathroomToiletTankAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBathroomToiletTankTex0  FILE=TEXTURES\TrolPipe_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBathroomToiletTankMesh NUM=0 TEXTURE=skBathroomToiletTankTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: TrolPipe_128.bmp  Path: C:\Harry Potter\ART\Objects\Bathroom\Troll\Pipe 


defaultproperties
{
    Mesh=skBathroomToiletTankMesh
    DrawType=DT_Mesh
    bStatic=False
}

