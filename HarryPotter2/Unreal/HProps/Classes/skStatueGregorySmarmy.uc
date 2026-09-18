//===============================================================================
//  [skStatueGregorySmarmy] 
//===============================================================================

class skStatueGregorySmarmy extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skStatueGregorySmarmyMesh MODELFILE=models\skStatueGregorySmarmy.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skStatueGregorySmarmyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skStatueGregorySmarmyAnims ANIMFILE=models\skStatueGregorySmarmy.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skStatueGregorySmarmyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skStatueGregorySmarmyMesh ANIM=skStatueGregorySmarmyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skStatueGregorySmarmyAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skStatueGregorySmarmyTex0  FILE=TEXTURES\gregory1.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skStatueGregorySmarmyMesh NUM=0 TEXTURE=skStatueGregorySmarmyTex0

// Original material [0] is [Material #13] SkinIndex: 0 Bitmap: gregory1.bmp  Path: \\Baker\HPotterPC\Art\Models\Objects\Hogwarts Props\Gregory 


defaultproperties
{
    Mesh=skStatueGregorySmarmyMesh
    DrawType=DT_Mesh
    bStatic=False
}

