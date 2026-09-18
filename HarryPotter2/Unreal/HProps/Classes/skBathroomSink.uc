//===============================================================================
//  [skBathroomSink] 
//===============================================================================

class skBathroomSink extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBathroomSinkMesh MODELFILE=models\skBathroomSink.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBathroomSinkMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBathroomSinkAnims ANIMFILE=models\skBathroomSink.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBathroomSinkMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBathroomSinkMesh ANIM=skBathroomSinkAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBathroomSinkAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBathroomSinkTex0  FILE=TEXTURES\TrollThrowSink.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBathroomSinkMesh NUM=0 TEXTURE=skBathroomSinkTex0

// Original material [0] is [sink_skinn00.MASKED] SkinIndex: 0 Bitmap: TrollThrowSink.bmp  Path: C:\Harry Potter\ART\Objects\Bathroom\Troll\Toilet_Sink 


defaultproperties
{
    Mesh=skBathroomSinkMesh
    DrawType=DT_Mesh
    bStatic=False
}

