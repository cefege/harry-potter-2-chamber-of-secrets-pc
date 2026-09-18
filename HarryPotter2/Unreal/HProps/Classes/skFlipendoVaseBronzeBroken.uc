//===============================================================================
//  [skFlipendoVaseBronzeBroken] 
//===============================================================================

class skFlipendoVaseBronzeBroken extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseBronzeBrokenMesh MODELFILE=models\skFlipendoVaseBronzeBroken.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseBronzeBrokenMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseBronzeBrokenAnims ANIMFILE=models\skFlipendoVaseBronzeBroken.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseBronzeBrokenMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseBronzeBrokenMesh ANIM=skFlipendoVaseBronzeBrokenAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseBronzeBrokenAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseBronzeBrokenTex0  FILE=TEXTURES\fvbrzbrk_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseBronzeBrokenMesh NUM=0 TEXTURE=skFlipendoVaseBronzeBrokenTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: fvbrzbrk_64.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseBronzeBrokenMesh
    DrawType=DT_Mesh
    bStatic=False
}

