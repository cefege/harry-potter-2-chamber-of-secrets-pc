//===============================================================================
//  [skFlipendoVaseMing] 
//===============================================================================

class skFlipendoVaseMing extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseMingMesh MODELFILE=models\skFlipendoVaseMing.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseMingMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseMingAnims ANIMFILE=models\skFlipendoVaseMing.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseMingMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseMingMesh ANIM=skFlipendoVaseMingAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseMingAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseMingTex0  FILE=TEXTURES\fvseming_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseMingMesh NUM=0 TEXTURE=skFlipendoVaseMingTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: fvseming_128.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo\Flipendo Vases 


defaultproperties
{
    Mesh=skFlipendoVaseMingMesh
    DrawType=DT_Mesh
    bStatic=False
	//brokentype=class'skFlipendoVaseMingBroken'
}

