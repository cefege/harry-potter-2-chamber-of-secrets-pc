//===============================================================================
//  [skTitleTest] 
//===============================================================================

class skTitleTest extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTitleTestMesh MODELFILE=models\skTitleTest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTitleTestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTitleTestAnims ANIMFILE=models\skTitleTest.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTitleTestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTitleTestMesh ANIM=skTitleTestAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTitleTestAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTitleTestTex0  FILE=TEXTURES\RoyalTapestry2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTitleTestMesh NUM=0 TEXTURE=skTitleTestTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: RoyalTapestry2.bmp  Path: C:\HP2 Art\Textures\Patterns 


defaultproperties
{
    Mesh=skTitleTestMesh
    DrawType=DT_Mesh
    bStatic=False
}

