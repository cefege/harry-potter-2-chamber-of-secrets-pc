//===============================================================================
//  [skBathroomToilet] 
//===============================================================================

class skBathroomToilet extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBathroomToiletMesh MODELFILE=models\skBathroomToilet.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBathroomToiletMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBathroomToiletAnims ANIMFILE=models\skBathroomToilet.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBathroomToiletMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBathroomToiletMesh ANIM=skBathroomToiletAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBathroomToiletAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBathroomToiletTex0  FILE=TEXTURES\TrollThrowToilet.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBathroomToiletMesh NUM=0 TEXTURE=skBathroomToiletTex0

// Original material [0] is [Toilet_skinn00] SkinIndex: 0 Bitmap: TrollThrowToilet.bmp  Path: C:\Harry Potter\ART\Objects\Bathroom\Troll\Toilet_Sink 


defaultproperties
{
    Mesh=skBathroomToiletMesh
    DrawType=DT_Mesh
    bStatic=False
}

