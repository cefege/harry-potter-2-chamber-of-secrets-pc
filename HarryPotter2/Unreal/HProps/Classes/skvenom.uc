//===============================================================================
//  [skvenom] 
//===============================================================================

class skvenom extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skvenomMesh MODELFILE=models\skvenom.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skvenomMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skvenomAnims ANIMFILE=models\skvenom.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skvenomMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skvenomMesh ANIM=skvenomAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skvenomAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skvenomTex0  FILE=TEXTURES\CVfall.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skvenomMesh NUM=0 TEXTURE=skvenomTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: CVfall.bmp  Path: C:\Harry Potter Art\Models 


defaultproperties
{
    Mesh=skvenomMesh
    DrawType=DT_Mesh
    bStatic=False
}

