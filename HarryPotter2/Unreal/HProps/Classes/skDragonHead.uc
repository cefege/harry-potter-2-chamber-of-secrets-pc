//===============================================================================
//  [skDragonHead] 
//===============================================================================

class skDragonHead extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDragonHeadMesh MODELFILE=models\skDragonHead.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDragonHeadMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDragonHeadAnims ANIMFILE=models\skDragonHead.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDragonHeadMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDragonHeadMesh ANIM=skDragonHeadAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDragonHeadAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDragonHeadTex0  FILE=TEXTURES\dragonsc_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDragonHeadMesh NUM=0 TEXTURE=skDragonHeadTex0

// Original material [0] is [Material #14] SkinIndex: 0 Bitmap: dragonsc_256.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Statues_Sculptures\Dragon Sculpture 


defaultproperties
{
    Mesh=skDragonHeadMesh
    DrawType=DT_Mesh
    bStatic=False
}

