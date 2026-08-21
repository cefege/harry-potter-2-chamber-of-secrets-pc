//===============================================================================
//  [skharrybed] 
//===============================================================================

class skharrybed extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skharrybedMesh MODELFILE=models\skharrybed.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skharrybedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skharrybedAnims ANIMFILE=models\skharrybed.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skharrybedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skharrybedMesh ANIM=skharrybedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skharrybedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skharrybedTex0  FILE=TEXTURES\harrybed_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skharrybedMesh NUM=0 TEXTURE=skharrybedTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: harrybed_128.bmp  Path: H:\Art\Models\Objects\Dursley Props\Beds 


defaultproperties
{
    Mesh=skharrybedMesh
    DrawType=DT_Mesh
    bStatic=False
}

