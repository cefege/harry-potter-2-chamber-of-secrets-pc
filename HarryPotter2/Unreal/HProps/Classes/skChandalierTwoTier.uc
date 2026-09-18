//===============================================================================
//  [skChandalierTwoTier] 
//===============================================================================

class skChandalierTwoTier extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChandalierTwoTierMesh MODELFILE=models\skChandalierTwoTier.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChandalierTwoTierMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChandalierTwoTierAnims ANIMFILE=models\skChandalierTwoTier.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChandalierTwoTierMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChandalierTwoTierMesh ANIM=skChandalierTwoTierAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChandalierTwoTierAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChandalierTwoTierTex0  FILE=TEXTURES\Chndaler_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChandalierTwoTierMesh NUM=0 TEXTURE=skChandalierTwoTierTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: Chndaler_128.bmp  Path: C:\Harry Potter\ART\Objects\Chandaliers\Tiered Brass 


defaultproperties
{
    Mesh=skChandalierTwoTierMesh
    DrawType=DT_Mesh
    bStatic=False
}

