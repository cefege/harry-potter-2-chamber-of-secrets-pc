//===============================================================================
//  [skChandalierSingleTier] 
//===============================================================================

class skChandalierSingleTier extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChandalierSingleTierMesh MODELFILE=models\skChandalierSingleTier.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChandalierSingleTierMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChandalierSingleTierAnims ANIMFILE=models\skChandalierSingleTier.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChandalierSingleTierMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChandalierSingleTierMesh ANIM=skChandalierSingleTierAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChandalierSingleTierAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChandalierSingleTierTex0  FILE=TEXTURES\Chndaler_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChandalierSingleTierMesh NUM=0 TEXTURE=skChandalierSingleTierTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: Chndaler_128.bmp  Path: C:\Harry Potter\ART\Objects\Chandaliers\Tiered Brass 


defaultproperties
{
    Mesh=skChandalierSingleTierMesh
    DrawType=DT_Mesh
    bStatic=False
}

