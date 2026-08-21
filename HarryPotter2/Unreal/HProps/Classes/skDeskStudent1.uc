//===============================================================================
//  [skDeskStudent1] 
//===============================================================================

class skDeskStudent1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDeskStudent1Mesh MODELFILE=models\skDeskStudent1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDeskStudent1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDeskStudent1Anims ANIMFILE=models\skDeskStudent1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDeskStudent1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDeskStudent1Mesh ANIM=skDeskStudent1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDeskStudent1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDeskStudent1Tex0  FILE=TEXTURES\StudentDesk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDeskStudent1Mesh NUM=0 TEXTURE=skDeskStudent1Tex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: StudentDesk_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Student Desk 


defaultproperties
{
    Mesh=skDeskStudent1Mesh
    DrawType=DT_Mesh
    bStatic=False
}

