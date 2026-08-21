//===============================================================================
//  [skSnakeRay] 
//===============================================================================

class skSnakeRay extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSnakeRayMesh MODELFILE=models\skSnakeRay.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSnakeRayMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSnakeRayAnims ANIMFILE=models\skSnakeRay.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSnakeRayMesh X=0.5 Y=0.5 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSnakeRayMesh ANIM=skSnakeRayAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSnakeRayAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSnakeRayTex0  FILE=TEXTURES\snakeeyes.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSnakeRayMesh NUM=0 TEXTURE=skSnakeRayTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: snakeeyes.bmp  Path: C:\Harry Potter Art\Models\snakeeyesmesh 


defaultproperties
{
    Mesh=skSnakeRayMesh
    DrawType=DT_Mesh
    bStatic=False
}

