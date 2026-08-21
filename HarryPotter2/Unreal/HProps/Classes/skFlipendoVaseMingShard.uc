//===============================================================================
//  [skFlipendoVaseMingShard] 
//===============================================================================

class skFlipendoVaseMingShard extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseMingShardMesh MODELFILE=models\skFlipendoVaseMingShard.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseMingShardMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseMingShardAnims ANIMFILE=models\skFlipendoVaseMingShard.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseMingShardMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseMingShardMesh ANIM=skFlipendoVaseMingShardAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseMingShardAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseMingShardTex0  FILE=TEXTURES\fvmingbk_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseMingShardMesh NUM=0 TEXTURE=skFlipendoVaseMingShardTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: fvmingbk_256.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseMingShardMesh
    DrawType=DT_Mesh
    bStatic=False
}

