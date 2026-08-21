//===============================================================================
//  [skBooksFlourishStand] 
//===============================================================================

class skBooksFlourishStand extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksFlourishStandMesh MODELFILE=models\skBooksFlourishStand.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksFlourishStandMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksFlourishStandAnims ANIMFILE=models\skBooksFlourishStand.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksFlourishStandMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksFlourishStandMesh ANIM=skBooksFlourishStandAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksFlourishStandAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksFlourishStandTex0  FILE=TEXTURES\BookStand_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksFlourishStandMesh NUM=0 TEXTURE=skBooksFlourishStandTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: BookStand_256.bmp  Path: C:\Harry Potter 2\ART\Objects\Books\Flourish Book Stand 


defaultproperties
{
    Mesh=skBooksFlourishStandMesh
    DrawType=DT_Mesh
    bStatic=False
}

