//===============================================================================
//  [skDiffindoRope256] 
//===============================================================================

class skDiffindoRope256 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoRope256Mesh MODELFILE=models\skDiffindoRope256.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoRope256Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoRope256Anims ANIMFILE=models\skDiffindoRope256.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoRope256Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoRope256Mesh ANIM=skDiffindoRope256Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoRope256Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoRope256Tex0  FILE=TEXTURES\DiffRope.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoRope256Mesh NUM=0 TEXTURE=skDiffindoRope256Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: DiffRope.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Rope 


defaultproperties
{
    Mesh=skDiffindoRope256Mesh
    DrawType=DT_Mesh
    bStatic=False
}

