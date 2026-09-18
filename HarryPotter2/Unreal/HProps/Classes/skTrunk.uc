//===============================================================================
//  [skTrunk] 
//===============================================================================

class skTrunk extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTrunkMesh MODELFILE=models\skTrunk.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTrunkMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTrunkAnims ANIMFILE=models\skTrunk.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTrunkMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTrunkMesh ANIM=skTrunkAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTrunkAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTrunkTex0  FILE=TEXTURES\DaigonTrunk.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTrunkMesh NUM=0 TEXTURE=skTrunkTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: DaigonTrunk.bmp  Path: C:\Harry Potter 2\ART\Objects\Chests_Boxes_Trunks\Daigon Trunks 


defaultproperties
{
    Mesh=skTrunkMesh
    DrawType=DT_Mesh
    bStatic=False
}

