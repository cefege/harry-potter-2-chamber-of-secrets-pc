//===============================================================================
//  [skFlipendoVaseMingBroken] 
//===============================================================================

class skFlipendoVaseMingBroken extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseMingBrokenMesh MODELFILE=models\skFlipendoVaseMingBroken.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseMingBrokenMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseMingBrokenAnims ANIMFILE=models\skFlipendoVaseMingBroken.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseMingBrokenMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseMingBrokenMesh ANIM=skFlipendoVaseMingBrokenAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseMingBrokenAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseMingBrokenTex0  FILE=TEXTURES\fvmingbk_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseMingBrokenMesh NUM=0 TEXTURE=skFlipendoVaseMingBrokenTex0

// Original material [0] is [mingvasebroken] SkinIndex: 0 Bitmap: fvmingbk_256.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseMingBrokenMesh
    DrawType=DT_Mesh
    bStatic=False
}

