//===============================================================================
//  [skSnakeBeam] 
//===============================================================================

class skSnakeBeam extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSnakeBeamMesh MODELFILE=models\skSnakeBeam.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSnakeBeamMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSnakeBeamAnims ANIMFILE=models\skSnakeBeam.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSnakeBeamMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSnakeBeamMesh ANIM=skSnakeBeamAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSnakeBeamAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSnakeBeamTex0  FILE=TEXTURES\snakeeyesGrey.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSnakeBeamMesh NUM=0 TEXTURE=skSnakeBeamTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: snakeeyes.bmp  Path: C:\HP_2\HProps\Textures 


defaultproperties
{
    Mesh=skSnakeBeamMesh
    DrawType=DT_Mesh
    bStatic=False
}

