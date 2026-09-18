//===============================================================================
//  [skFlipendoVaseGreenBroken] 
//===============================================================================

class skFlipendoVaseGreenBroken extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseGreenBrokenMesh MODELFILE=models\skFlipendoVaseGreenBroken.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseGreenBrokenMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseGreenBrokenAnims ANIMFILE=models\skFlipendoVaseGreenBroken.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseGreenBrokenMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseGreenBrokenMesh ANIM=skFlipendoVaseGreenBrokenAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseGreenBrokenAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseGreenBrokenTex0  FILE=TEXTURES\fvgrnbrk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseGreenBrokenMesh NUM=0 TEXTURE=skFlipendoVaseGreenBrokenTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: fvgrnbrk_128.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseGreenBrokenMesh
    DrawType=DT_Mesh
    bStatic=False
}

