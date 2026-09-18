//===============================================================================
//  [skDiffindoRope128] 
//===============================================================================

class skDiffindoRope128 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoRope128Mesh MODELFILE=models\skDiffindoRope128.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoRope128Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoRope128Anims ANIMFILE=models\skDiffindoRope128.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoRope128Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoRope128Mesh ANIM=skDiffindoRope128Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoRope128Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoRope128Tex0  FILE=TEXTURES\DiffRope.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoRope128Mesh NUM=0 TEXTURE=skDiffindoRope128Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: DiffRope.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Rope 


defaultproperties
{
    Mesh=skDiffindoRope128Mesh
    DrawType=DT_Mesh
    bStatic=False
}

