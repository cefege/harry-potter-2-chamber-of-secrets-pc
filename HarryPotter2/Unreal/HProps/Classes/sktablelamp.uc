//===============================================================================
//  [sktablelamp] 
//===============================================================================

class sktablelamp extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=sktablelampMesh MODELFILE=models\sktablelamp.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=sktablelampMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=sktablelampAnims ANIMFILE=models\sktablelamp.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=sktablelampMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=sktablelampMesh ANIM=sktablelampAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=sktablelampAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=sktablelampTex0  FILE=TEXTURES\Lamps_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=sktablelampMesh NUM=0 TEXTURE=sktablelampTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: Lamps_128.bmp  Path: H:\Art\Models\Objects\Dursley Props\Lamp Table 


defaultproperties
{
    Mesh=sktablelampMesh
    DrawType=DT_Mesh
    bStatic=False
}

