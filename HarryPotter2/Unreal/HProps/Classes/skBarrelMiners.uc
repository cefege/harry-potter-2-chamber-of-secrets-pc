//===============================================================================
//  [skBarrelMiners] 
//===============================================================================

class skBarrelMiners extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBarrelMinersMesh MODELFILE=models\skBarrelMiners.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBarrelMinersMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBarrelMinersAnims ANIMFILE=models\skBarrelMiners.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBarrelMinersMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBarrelMinersMesh ANIM=skBarrelMinersAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBarrelMinersAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBarrelMinersTex0  FILE=TEXTURES\MinersBarrel.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBarrelMinersMesh NUM=0 TEXTURE=skBarrelMinersTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: MinersBarrel.bmp  Path: C:\Harry Potter\ART\Objects\Barrels\Cave Barrel 


defaultproperties
{
    Mesh=skBarrelMinersMesh
    DrawType=DT_Mesh
    bStatic=False
}

