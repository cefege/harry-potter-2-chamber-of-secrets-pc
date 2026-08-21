//===============================================================================
//  [skCauldronTeacher] 
//===============================================================================

class skCauldronTeacher extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCauldronTeacherMesh MODELFILE=models\skCauldronTeacher.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCauldronTeacherMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCauldronTeacherAnims ANIMFILE=models\skCauldronTeacher.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCauldronTeacherMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCauldronTeacherMesh ANIM=skCauldronTeacherAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCauldronTeacherAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCauldronTeacherTex0  FILE=TEXTURES\teccould_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCauldronTeacherMesh NUM=0 TEXTURE=skCauldronTeacherTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: teccould_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Cauldrons\Teachers Cauldron 


defaultproperties
{
    Mesh=skCauldronTeacherMesh
    DrawType=DT_Mesh
    bStatic=False
}

