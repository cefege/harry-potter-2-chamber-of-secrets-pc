//===============================================================================
//  [skLampWall] 
//===============================================================================

class skLampWall extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLampWallMesh MODELFILE=models\skLampWall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLampWallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLampWallAnims ANIMFILE=models\skLampWall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLampWallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLampWallMesh ANIM=skLampWallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLampWallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLampWallTex0  FILE=TEXTURES\pntylamp_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLampWallMesh NUM=0 TEXTURE=skLampWallTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: pntylamp_128.bmp  Path: C:\Harry Potter\ART\Objects\Lanterns_Lamps\peaked lantern 


defaultproperties
{
    Mesh=skLampWallMesh
    DrawType=DT_Mesh
    bStatic=False
}

