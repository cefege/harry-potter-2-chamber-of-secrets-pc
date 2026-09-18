//===============================================================================
//  [skChairsSheetDraped] 
//===============================================================================

class skChairsSheetDraped extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsSheetDrapedMesh MODELFILE=models\skChairsSheetDraped.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsSheetDrapedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsSheetDrapedAnims ANIMFILE=models\skChairsSheetDraped.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsSheetDrapedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsSheetDrapedMesh ANIM=skChairsSheetDrapedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsSheetDrapedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsSheetDrapedTex0  FILE=TEXTURES\chairsheet_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsSheetDrapedMesh NUM=0 TEXTURE=skChairsSheetDrapedTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: chairsheet_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Sheet Draped Chair 


defaultproperties
{
    Mesh=skChairsSheetDrapedMesh
    DrawType=DT_Mesh
    bStatic=False
}

