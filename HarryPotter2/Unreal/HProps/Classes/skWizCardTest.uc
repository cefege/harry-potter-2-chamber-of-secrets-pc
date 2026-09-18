//===============================================================================
//  [skWizCardTest] 
//===============================================================================

class skWizCardTest extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skWizCardTestMesh MODELFILE=models\skWizCardTest.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWizCardTestMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWizCardTestAnims ANIMFILE=models\skWizCardTest.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWizCardTestMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWizCardTestMesh ANIM=skWizCardTestAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWizCardTestAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skWizCardTestTex0  FILE=TEXTURES\ForeGround.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWizCardTestTex1  FILE=TEXTURES\MiddleGround.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWizCardTestTex2  FILE=TEXTURES\Wiznight.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWizCardTestMesh NUM=0 TEXTURE=skWizCardTestTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skWizCardTestMesh NUM=1 TEXTURE=skWizCardTestTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skWizCardTestMesh NUM=2 TEXTURE=skWizCardTestTex2

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: ForeGround.bmp  Path: C:\HP2 Art\Textures 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: MiddleGround.bmp  Path: C:\HP2 Art\Textures 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: Wiznight.bmp  Path: C:\HP2 Art\Textures 


defaultproperties
{
    Mesh=skWizCardTestMesh
    DrawType=DT_Mesh
    bStatic=False
}

