//===============================================================================
//  [skChairsTeacher] 
//===============================================================================

class skChairsTeacher extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsTeacherMesh MODELFILE=models\skChairsTeacher.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsTeacherMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsTeacherAnims ANIMFILE=models\skChairsTeacher.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsTeacherMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsTeacherMesh ANIM=skChairsTeacherAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsTeacherAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsTeacherTex0  FILE=TEXTURES\TeacherChair_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsTeacherMesh NUM=0 TEXTURE=skChairsTeacherTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: TeacherChair_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Teacher Chair 


defaultproperties
{
    Mesh=skChairsTeacherMesh
    DrawType=DT_Mesh
    bStatic=False
}

