//===============================================================================
//  [skFlipendoVaseGreen] 
//===============================================================================

class skFlipendoVaseGreen extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseGreenMesh MODELFILE=models\skFlipendoVaseGreen.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseGreenMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseGreenAnims ANIMFILE=models\skFlipendoVaseGreen.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseGreenMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseGreenMesh ANIM=skFlipendoVaseGreenAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseGreenAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseGreenTex0  FILE=TEXTURES\fvasegrn_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseGreenMesh NUM=0 TEXTURE=skFlipendoVaseGreenTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: fvasegrn_64.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseGreenMesh
    DrawType=DT_Mesh
    bStatic=False
	//brokentype=class'skFlipendoVaseGreenBroken'
}

