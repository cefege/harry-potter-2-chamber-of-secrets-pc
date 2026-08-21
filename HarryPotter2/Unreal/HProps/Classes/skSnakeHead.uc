//===============================================================================
//  [skSnakeHead] 
//===============================================================================

class skSnakeHead extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSnakeHeadMesh MODELFILE=models\skSnakeHead.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSnakeHeadMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSnakeHeadAnims ANIMFILE=models\skSnakeHead.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSnakeHeadMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSnakeHeadMesh ANIM=skSnakeHeadAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSnakeHeadAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSnakeHeadTex0  FILE=TEXTURES\COSsnakehead_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSnakeHeadTex1  FILE=TEXTURES\COSsnakehead_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSnakeHeadMesh NUM=0 TEXTURE=skSnakeHeadTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skSnakeHeadMesh NUM=1 TEXTURE=skSnakeHeadTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: COSsnakehead_SKIN00.bmp  Path: C:\Harry Potter 2\ART\Objects\Statues_Sculptures\Snake Head 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: COSsnakehead_SKIN01.bmp  Path: C:\Harry Potter 2\ART\Objects\Statues_Sculptures\Snake Head 


defaultproperties
{
    Mesh=skSnakeHeadMesh
    DrawType=DT_Mesh
    bStatic=False
}

