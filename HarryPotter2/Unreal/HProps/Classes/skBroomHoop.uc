//===============================================================================
//  [skBroomHoop] 
//===============================================================================

class skBroomHoop extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBroomHoopMesh MODELFILE=models\skBroomHoop.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBroomHoopMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBroomHoopAnims ANIMFILE=models\skBroomHoop.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBroomHoopMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBroomHoopMesh ANIM=skBroomHoopAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBroomHoopAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBroomHoopTex0  FILE=TEXTURES\line.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBroomHoopMesh NUM=0 TEXTURE=skBroomHoopTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: line.bmp  Path: D:\Harry Potter\Art\Objects\General Objects\Qudditch 


defaultproperties
{
    Mesh=skBroomHoopMesh
    DrawType=DT_Mesh
    bStatic=False
}

