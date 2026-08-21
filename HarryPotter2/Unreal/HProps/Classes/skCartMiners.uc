//===============================================================================
//  [skCartMiners] 
//===============================================================================

class skCartMiners extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCartMinersMesh MODELFILE=models\skCartMiners.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCartMinersMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCartMinersAnims ANIMFILE=models\skCartMiners.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCartMinersMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCartMinersMesh ANIM=skCartMinersAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCartMinersAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCartMinersTex0  FILE=TEXTURES\MinersCart.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCartMinersMesh NUM=0 TEXTURE=skCartMinersTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: MinersCart.bmp  Path: C:\Harry Potter\ART\Objects\Carts_Wagons\Cave Mine Cart 


defaultproperties
{
    Mesh=skCartMinersMesh
    DrawType=DT_Mesh
    bStatic=False
}

