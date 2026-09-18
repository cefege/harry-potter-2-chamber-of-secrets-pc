//===============================================================================
//  [skDiffindoRope64] 
//===============================================================================

class skDiffindoRope64 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoRope64Mesh MODELFILE=models\skDiffindoRope64.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoRope64Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoRope64Anims ANIMFILE=models\skDiffindoRope64.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoRope64Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoRope64Mesh ANIM=skDiffindoRope64Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoRope64Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoRope64Tex0  FILE=TEXTURES\DiffRope.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoRope64Mesh NUM=0 TEXTURE=skDiffindoRope64Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: DiffRope.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Rope 


defaultproperties
{
    Mesh=skDiffindoRope64Mesh
    DrawType=DT_Mesh
    bStatic=False
}

