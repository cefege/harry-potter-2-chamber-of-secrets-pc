//===============================================================================
//  [skCutSceneMarker] 
//===============================================================================

class skCutSceneMarker extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCutSceneMarkerMesh MODELFILE=models\skCutSceneMarker.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCutSceneMarkerMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCutSceneMarkerAnims ANIMFILE=models\skCutSceneMarker.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCutSceneMarkerMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCutSceneMarkerMesh ANIM=skCutSceneMarkerAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCutSceneMarkerAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCutSceneMarkerTex0  FILE=TEXTURES\cutscene_tex.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCutSceneMarkerMesh NUM=0 TEXTURE=skCutSceneMarkerTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: cutscene_tex.bmp  Path: C:\Harry Potter 2\ART\Objects\Engine Objects 


defaultproperties
{
    Mesh=skCutSceneMarkerMesh
    DrawType=DT_Mesh
    bStatic=False
}

