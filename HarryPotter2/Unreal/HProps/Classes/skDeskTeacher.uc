//===============================================================================
//  [skDeskTeacher] 
//===============================================================================

class skDeskTeacher extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDeskTeacherMesh MODELFILE=models\skDeskTeacher.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDeskTeacherMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDeskTeacherAnims ANIMFILE=models\skDeskTeacher.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDeskTeacherMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDeskTeacherMesh ANIM=skDeskTeacherAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDeskTeacherAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDeskTeacherTex0  FILE=TEXTURES\TeacherDesk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDeskTeacherMesh NUM=0 TEXTURE=skDeskTeacherTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: TeacherDesk_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Teacher Desk 


defaultproperties
{
    Mesh=skDeskTeacherMesh
    DrawType=DT_Mesh
    bStatic=False
}

