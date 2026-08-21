//===============================================================================
//  [skDeskStudentWSeat] 
//===============================================================================

class skDeskStudentWSeat extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDeskStudentWSeatMesh MODELFILE=models\skDeskStudentWSeat.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDeskStudentWSeatMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDeskStudentWSeatAnims ANIMFILE=models\skDeskStudentWSeat.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDeskStudentWSeatMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDeskStudentWSeatMesh ANIM=skDeskStudentWSeatAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDeskStudentWSeatAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDeskStudentWSeatTex0  FILE=TEXTURES\StudentDesk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDeskStudentWSeatMesh NUM=0 TEXTURE=skDeskStudentWSeatTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: StudentDesk_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Student Desk 


defaultproperties
{
    Mesh=skDeskStudentWSeatMesh
    DrawType=DT_Mesh
    bStatic=False
}

