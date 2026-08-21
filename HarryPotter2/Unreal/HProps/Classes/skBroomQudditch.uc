//===============================================================================
//  [skBroomQudditch] 
//===============================================================================

class skBroomQudditch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBroomQudditchMesh MODELFILE=models\skBroomQudditch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBroomQudditchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBroomQudditchAnims ANIMFILE=models\skBroomQudditch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBroomQudditchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBroomQudditchMesh ANIM=skBroomQudditchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBroomQudditchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBroomQudditchTex0  FILE=TEXTURES\QUID_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBroomQudditchMesh NUM=0 TEXTURE=skBroomQudditchTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: QUID_SKIN01.bmp  Path: C:\Harry Potter 2\ART\Objects\Brooms_Mops_Buckets\Quidditch Broom 


defaultproperties
{
    Mesh=skBroomQudditchMesh
    DrawType=DT_Mesh
    bStatic=False
}

