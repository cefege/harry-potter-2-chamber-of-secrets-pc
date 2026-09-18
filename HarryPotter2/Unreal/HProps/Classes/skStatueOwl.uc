//===============================================================================
//  [skStatueOwl] 
//===============================================================================

class skStatueOwl extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skStatueOwlMesh MODELFILE=models\skStatueOwl.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skStatueOwlMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skStatueOwlAnims ANIMFILE=models\skStatueOwl.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skStatueOwlMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skStatueOwlMesh ANIM=skStatueOwlAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skStatueOwlAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skStatueOwlTex0  FILE=TEXTURES\OwlStatue.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skStatueOwlMesh NUM=0 TEXTURE=skStatueOwlTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: OwlStatue.bmp  Path: C:\Harry Potter\ART\Objects\Statues_Sculptures\Owl Statue 


defaultproperties
{
    Mesh=skStatueOwlMesh
    DrawType=DT_Mesh
    bStatic=False
}

