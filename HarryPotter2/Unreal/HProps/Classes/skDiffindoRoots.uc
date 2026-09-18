//===============================================================================
//  [skDiffindoRoots] 
//===============================================================================

class skDiffindoRoots extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoRootsMesh MODELFILE=models\skDiffindoRoots.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoRootsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoRootsAnims ANIMFILE=models\skDiffindoRoots.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoRootsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoRootsMesh ANIM=skDiffindoRootsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoRootsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoRootsTex0  FILE=TEXTURES\DiffRoots.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoRootsMesh NUM=0 TEXTURE=skDiffindoRootsTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: DiffRoots.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Roots 


defaultproperties
{
    Mesh=skDiffindoRootsMesh
    DrawType=DT_Mesh
    bStatic=False
}

