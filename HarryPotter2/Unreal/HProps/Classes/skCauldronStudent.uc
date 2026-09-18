//===============================================================================
//  [skCauldronStudent] 
//===============================================================================

class skCauldronStudent extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCauldronStudentMesh MODELFILE=models\skCauldronStudent.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCauldronStudentMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCauldronStudentAnims ANIMFILE=models\skCauldronStudent.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCauldronStudentMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCauldronStudentMesh ANIM=skCauldronStudentAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCauldronStudentAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCauldronStudentTex0  FILE=TEXTURES\stucould_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCauldronStudentMesh NUM=0 TEXTURE=skCauldronStudentTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: stucould_128.bmp  Path: C:\Harry Potter\ART\Objects\Cauldrons\Student Cauldron 


defaultproperties
{
    Mesh=skCauldronStudentMesh
    DrawType=DT_Mesh
    bStatic=False
}

