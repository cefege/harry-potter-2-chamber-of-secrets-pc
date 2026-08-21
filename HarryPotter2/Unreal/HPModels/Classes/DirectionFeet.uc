//===============================================================================
//  [DirectionFeet] 
//===============================================================================

class DirectionFeet extends Actor;
#exec MESH  MODELIMPORT MESH=DirectionFeetMesh MODELFILE=models\DirectionFeet.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=DirectionFeetMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=DirectionFeetMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=DirectionFeetMesh ANIM=NoAnims

#EXEC TEXTURE IMPORT NAME=DirectionFeetTex0  FILE=TEXTURES\feet.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=DirectionFeetMesh NUM=0 TEXTURE=DirectionFeetTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: feet.bmp  Path: D:\Harry Potter\Art\Objects\General Objects\project objects 


defaultproperties
{
    Mesh=DirectionFeetMesh
    DrawType=DT_Mesh
    bStatic=False
}

