//===============================================================================
//  [skBooksFourStacked] 
//===============================================================================

class skBooksFourStacked extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksFourStackedMesh MODELFILE=models\skBooksFourStacked.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksFourStackedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksFourStackedAnims ANIMFILE=models\skBooksFourStacked.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksFourStackedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksFourStackedMesh ANIM=skBooksFourStackedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksFourStackedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksFourStackedTex0  FILE=TEXTURES\BookStack_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksFourStackedMesh NUM=0 TEXTURE=skBooksFourStackedTex0

// Original material [0] is [Material #7] SkinIndex: 0 Bitmap: BookStack_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Four Flat Stacked 


defaultproperties
{
    Mesh=skBooksFourStackedMesh
    DrawType=DT_Mesh
    bStatic=False
}

