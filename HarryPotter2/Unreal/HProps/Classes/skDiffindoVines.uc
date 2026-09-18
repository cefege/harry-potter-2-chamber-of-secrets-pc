//===============================================================================
//  [skDiffindoVines] 
//===============================================================================

class skDiffindoVines extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoVinesMesh MODELFILE=models\skDiffindoVines.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoVinesMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoVinesAnims ANIMFILE=models\skDiffindoVines.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoVinesMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoVinesMesh ANIM=skDiffindoVinesAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoVinesAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoVinesTex0  FILE=TEXTURES\DiffVines.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDiffindoVinesTex1  FILE=TEXTURES\DiffVines.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoVinesMesh NUM=0 TEXTURE=skDiffindoVinesTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoVinesMesh NUM=1 TEXTURE=skDiffindoVinesTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: DiffVines.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Vines 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: DiffVines.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Vines 


defaultproperties
{
    Mesh=skDiffindoVinesMesh
    DrawType=DT_Mesh
    bStatic=False
}

