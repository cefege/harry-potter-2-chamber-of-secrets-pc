//===============================================================================
//  [skFlipendoVaseGreenShard] 
//===============================================================================

class skFlipendoVaseGreenShard extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseGreenShardMesh MODELFILE=models\skFlipendoVaseGreenShard.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseGreenShardMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseGreenShardAnims ANIMFILE=models\skFlipendoVaseGreenShard.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseGreenShardMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseGreenShardMesh ANIM=skFlipendoVaseGreenShardAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseGreenShardAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseGreenShardTex0  FILE=TEXTURES\fvgrnbrk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseGreenShardMesh NUM=0 TEXTURE=skFlipendoVaseGreenShardTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: fvgrnbrk_128.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseGreenShardMesh
    DrawType=DT_Mesh
    bStatic=False
}

