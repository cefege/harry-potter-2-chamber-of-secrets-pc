//===============================================================================
//  [skChandalierThreeTier] 
//===============================================================================

class skChandalierThreeTier extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChandalierThreeTierMesh MODELFILE=models\skChandalierThreeTier.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChandalierThreeTierMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChandalierThreeTierAnims ANIMFILE=models\skChandalierThreeTier.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChandalierThreeTierMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChandalierThreeTierMesh ANIM=skChandalierThreeTierAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChandalierThreeTierAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChandalierThreeTierTex0  FILE=TEXTURES\Chndaler_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChandalierThreeTierMesh NUM=0 TEXTURE=skChandalierThreeTierTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: Chndaler_128.bmp  Path: C:\Harry Potter\ART\Objects\Chandaliers\Tiered Brass 


defaultproperties
{
    Mesh=skChandalierThreeTierMesh
    DrawType=DT_Mesh
    bStatic=False
}

