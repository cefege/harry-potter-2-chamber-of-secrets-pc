//===============================================================================
//  [skRope] 
//===============================================================================

class skRope extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skRopeMesh MODELFILE=models\skRope.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skRopeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skRopeAnims ANIMFILE=models\skRope.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skRopeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skRopeMesh ANIM=skRopeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skRopeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skRopeTex0  FILE=TEXTURES\Rope.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skRopeMesh NUM=0 TEXTURE=skRopeTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: skRope.bmp  Path: C:\Harry Potter\ART\Objects\skRopes_Chains\Cave skRope 


defaultproperties
{
    Mesh=skRopeMesh
    DrawType=DT_Mesh
    bStatic=False
}

