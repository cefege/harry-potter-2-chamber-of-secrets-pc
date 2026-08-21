//===============================================================================
//  [skBedGryffindor] 
//===============================================================================

class skBedGryffindor extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBedGryffindorMesh MODELFILE=models\skBedGryffindor.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBedGryffindorMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBedGryffindorAnims ANIMFILE=models\skBedGryffindor.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBedGryffindorMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBedGryffindorMesh ANIM=skBedGryffindorAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBedGryffindorAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBedGryffindorTex0  FILE=TEXTURES\gryffbed_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBedGryffindorMesh NUM=0 TEXTURE=skBedGryffindorTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: gryffbed_128.bmp  Path: C:\Harry Potter\ART\Objects\Beds\Gryffindor Bed 


defaultproperties
{
    Mesh=skBedGryffindorMesh
    DrawType=DT_Mesh
    bStatic=False
}

