//===============================================================================
//  [skChairsDumbledoresThrone] 
//===============================================================================

class skChairsDumbledoresThrone extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsDumbledoresThroneMesh MODELFILE=models\skChairsDumbledoresThrone.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsDumbledoresThroneMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsDumbledoresThroneAnims ANIMFILE=models\skChairsDumbledoresThrone.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsDumbledoresThroneMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsDumbledoresThroneMesh ANIM=skChairsDumbledoresThroneAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsDumbledoresThroneAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsDumbledoresThroneTex0  FILE=TEXTURES\ghthrone_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsDumbledoresThroneMesh NUM=0 TEXTURE=skChairsDumbledoresThroneTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: ghthrone_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Dumbledore Throne 


defaultproperties
{
    Mesh=skChairsDumbledoresThroneMesh
    DrawType=DT_Mesh
    bStatic=False
}

