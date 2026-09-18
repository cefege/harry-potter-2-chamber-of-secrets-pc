//===============================================================================
//  [skStatueDragon] 
//===============================================================================

class skStatueDragon extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skStatueDragonMesh MODELFILE=models\skStatueDragon.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skStatueDragonMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skStatueDragonAnims ANIMFILE=models\skStatueDragon.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skStatueDragonMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skStatueDragonMesh ANIM=skStatueDragonAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skStatueDragonAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skStatueDragonTex0  FILE=TEXTURES\dragonsc_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skStatueDragonMesh NUM=0 TEXTURE=skStatueDragonTex0

// Original material [0] is [Material #14] SkinIndex: 0 Bitmap: dragonsc_256.bmp  Path: C:\Harry Potter\ART\Objects\Statues_Sculptures\Dragon Sculpture 


defaultproperties
{
    Mesh=skStatueDragonMesh
    DrawType=DT_Mesh
    bStatic=False
}

