//===============================================================================
//  [DirectionCamera] 
//===============================================================================

class DirectionCamera extends actor;
#exec MESH  MODELIMPORT MESH=DirectionCameraMesh MODELFILE=models\DirectionCamera.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=DirectionCameraMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=DirectionCameraAnims ANIMFILE=models\DirectionCamera.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=DirectionCameraMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=DirectionCameraMesh ANIM=DirectionCameraAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=DirectionCameraAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=DirectionCameraTex0  FILE=TEXTURES\camera.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=DirectionCameraMesh NUM=0 TEXTURE=DirectionCameraTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: camera.bmp  Path: D:\Harry Potter\Art\Objects\General Objects\project objects 


defaultproperties
{
    Mesh=DirectionCameraMesh
    DrawType=DT_Mesh
    bStatic=False
}

